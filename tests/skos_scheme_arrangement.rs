use {
    anyhow::Result,
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
    scraper::{Html, Selector},
    std::path::Path,
};

async fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    tokio::fs::create_dir_all(target).await?;
    let mut entries = tokio::fs::read_dir(source).await?;
    while let Some(entry) = entries.next_entry().await? {
        let destination = target.join(entry.file_name());
        if entry.file_type().await?.is_dir() {
            Box::pin(copy_tree(&entry.path(), &destination)).await?;
        } else {
            tokio::fs::copy(entry.path(), destination).await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn skos_concepts_are_nested_in_schemes_with_one_canonical_entry_after_restore()
-> Result<()> {
    let source = tempfile::tempdir()?;
    copy_tree(Path::new("examples/dprod"), source.path()).await?;
    let path = source.path().join("sources/ontology.ttl");
    let mut rdf = tokio::fs::read_to_string(&path).await?;
    rdf.push_str(r#"
dprod:DataContractLifecycleStatus a skos:ConceptScheme;
 rdfs:label "Contract lifecycle statuses"@en; dct:description "Statuses of a contract."@en;
 skos:hasTopConcept dprod:Pending; rdfs:isDefinedBy dprod: .
dprod:ReviewStatuses a skos:ConceptScheme; rdfs:label "Review statuses"@en;
 dct:description "Statuses used in review."@en; rdfs:isDefinedBy dprod: .
dprod:Pending a skos:Concept; rdfs:label "Pending"@en; dct:description "Not yet active."@en; rdfs:isDefinedBy dprod: .
dprod:Active a skos:Concept; rdfs:label "Active"@en; dct:description "Currently active."@en; skos:inScheme dprod:DataContractLifecycleStatus; skos:broader dprod:Pending; rdfs:isDefinedBy dprod: .
dprod:Fulfilled a skos:Concept; rdfs:label "Fulfilled"@en; dct:description "Completed."@en; skos:topConceptOf dprod:DataContractLifecycleStatus; rdfs:isDefinedBy dprod: .
dprod:Violated a skos:Concept; rdfs:label "Violated"@en; dct:description "Not fulfilled."@en; skos:inScheme dprod:DataContractLifecycleStatus, dprod:ReviewStatuses; rdfs:isDefinedBy dprod: .
dprod:Unassigned a skos:Concept; rdfs:label "Unassigned"@en; dct:description "Without an asserted scheme."@en; skos:broader dprod:Pending; rdfs:isDefinedBy dprod: .
"#);
    tokio::fs::write(path, rdf).await?;
    let (dataset, plans) = showcases::dprod(source.path()).await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    showcases::publish(&Publisher::new(restored), &plans, output.path())
        .await?;
    let html = tokio::fs::read_to_string(
        output.path().join("specification/index.html"),
    )
    .await?;
    let doc = Html::parse_document(&html);
    let headings = doc
        .select(
            &Selector::parse(
                "main h1, main h2, main h3, main h4, article h1, article h2, \
                 article h3, article h4",
            )
            .unwrap(),
        )
        .map(|h| {
            (
                h.value().name().to_owned(),
                h.text().collect::<String>(),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        headings.iter().any(|(level, title)| {
            level == "h4" && title.contains("dprod:Pending")
        }),
        "scheme members need a deeper heading: {headings:?}"
    );
    assert!(
        !headings
            .iter()
            .any(|(_, title)| title.contains("SKOS concepts")),
        "concepts should not be a sibling chapter of schemes"
    );
    assert!(html.contains("Concepts without a documented scheme"));
    assert!(
        html.contains("Also listed in"),
        "multiple scheme membership must be explicit"
    );
    let typst =
        tokio::fs::read_to_string(output.path().join("pdf/main.typ")).await?;
    assert!(typst.contains("Contract lifecycle statuses"));
    assert!(typst.contains("Concepts without a documented scheme"));
    for label in ["Pending", "Active", "Fulfilled", "Violated"] {
        assert!(typst.contains(label));
    }
    assert!(
        tokio::fs::read(output.path().join("pdf/document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn skos_cycles_are_review_findings_with_finite_linked_navigation()
-> Result<()> {
    let source = tempfile::tempdir()?;
    copy_tree(Path::new("examples/dprod"), source.path()).await?;
    let path = source.path().join("sources/ontology.ttl");
    let mut rdf = tokio::fs::read_to_string(&path).await?;
    rdf.push_str(r#"
dprod:CycleScheme a skos:ConceptScheme; rdfs:label "Cyclic scheme"@en; dct:description "An intentionally inconsistent test hierarchy."@en; rdfs:isDefinedBy dprod: .
dprod:CycleA a skos:Concept; rdfs:label "Cycle A"@en; dct:description "First concept."@en; skos:inScheme dprod:CycleScheme; skos:broader dprod:CycleB; rdfs:isDefinedBy dprod: .
dprod:CycleB a skos:Concept; rdfs:label "Cycle B"@en; dct:description "Second concept."@en; skos:inScheme dprod:CycleScheme; skos:narrower dprod:CycleA; skos:broader dprod:CycleA; rdfs:isDefinedBy dprod: .
"#);
    tokio::fs::write(path, rdf).await?;
    let (dataset, plans) = showcases::dprod(source.path()).await?;
    let output = tempfile::tempdir()?;
    let publications =
        showcases::publish(&Publisher::new(dataset), &plans, output.path())
            .await?;
    assert!(publications.iter().any(|publication| {
        publication
            .manifest
            .diagnostics
            .iter()
            .any(|issue| issue.code == "skos-hierarchy-cycle")
    }));
    let html = tokio::fs::read_to_string(
        output.path().join("specification/index.html"),
    )
    .await?;
    assert!(
        html.contains("Concept hierarchy cycles"),
        "cycle warning also belongs in the quality appendix"
    );
    assert!(html.contains("without inventing a tree"));
    assert!(
        html.len() < 1_000_000,
        "cycles must not recursively expand the published hierarchy"
    );
    Ok(())
}

#[tokio::test]
async fn skos_schemes_can_share_a_source_namespace_and_concept_without_duplicate_entries()
-> Result<()> {
    for hybrid in [false, true] {
        let source = tempfile::tempdir()?;
        copy_tree(Path::new("examples/skos"), source.path()).await?;
        let project = source.path().join("project.ttl");
        let mut definitions = tokio::fs::read_to_string(&project).await?;
        definitions.push_str(
            "\n<https://example.org/data-taxonomy-review> gov:inFamily \
             <urn:skos:family> .\n",
        );
        tokio::fs::write(project, definitions).await?;
        let path = source.path().join("sources/taxonomy.ttl");
        let mut rdf = tokio::fs::read_to_string(&path).await?;
        rdf.push_str(r#"
<https://example.org/data-taxonomy-review> a skos:ConceptScheme;
 skos:prefLabel "Review taxonomy"@en;skos:definition "A second scheme sharing concepts."@en;
 sh:declare [sh:prefix "data-taxonomy";sh:namespace "https://example.org/data-taxonomy/"];
 skos:hasTopConcept ex:Product .
ex:Unassigned a skos:Concept;skos:prefLabel "Unassigned concept"@en;
 skos:definition "A concept without asserted scheme membership."@en;skos:broader ex:Product .
"#);
        if hybrid {
            rdf.push_str("\n<https://example.org/data-taxonomy> a <http://www.w3.org/2002/07/owl#Ontology> .\n");
        }
        tokio::fs::write(path, rdf).await?;
        let (dataset, plans) = showcases::skos(source.path()).await?;
        let output = tempfile::tempdir()?;
        let archive = output.path().join("kg.nq");
        dataset.export(archive.clone()).await?;
        source.close()?;
        let restored = Dataset::memory()?;
        restored.import(archive).await?;
        showcases::publish(&Publisher::new(restored), &plans, output.path())
            .await?;
        let html = tokio::fs::read_to_string(
            output.path().join("specification/index.html"),
        )
        .await?;
        let doc = Html::parse_document(&html);
        let headings = doc
            .select(
                &Selector::parse("main h2,main h3,main h4,main h5").unwrap(),
            )
            .map(|h| h.text().collect::<String>())
            .collect::<Vec<_>>();
        for label in ["Product§", "Unassigned concept§"] {
            assert_eq!(
                headings
                    .iter()
                    .filter(|heading| heading.ends_with(label))
                    .count(),
                1,
                "one complete reference entry per concept: {headings:?}"
            );
        }
        assert!(html.contains("Review taxonomy"));
        assert!(html.contains("reference entry in another scheme"));
        assert!(html.contains("Concepts without a documented scheme"));
    }
    Ok(())
}
