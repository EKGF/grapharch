use {
    super::*,
    crate::publication::collect::{Collector, RdfFile},
};

const PROJECT: &str = "urn:test:project";
const PREFIXES: &str = "@prefix ga: <https://ekgf.org/ontology/grapharch#> .\n\
    @prefix dct: <http://purl.org/dc/terms/> .\n\
    @prefix owl: <http://www.w3.org/2002/07/owl#> .\n\
    @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
    @prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
    @prefix sh: <http://www.w3.org/ns/shacl#> .\n\
    @prefix voc: <urn:voc:> .\n\
    @prefix t: <urn:test:> .\n";

/// A vocabulary whose terms are annotated by its source.
const CONTENT: &str = r#"
<urn:voc:> a owl:Ontology ; rdfs:label "Vocabulary"@en ;
    sh:declare [ sh:prefix "voc" ; sh:namespace "urn:voc:" ] .
voc:Thing a owl:Class ; rdfs:label "Thing"@en ; dct:description "A thing."@en ; rdfs:isDefinedBy <urn:voc:> .
voc:Both a owl:Class, owl:NamedIndividual ; rdfs:label "Both"@en ; dct:description "Typed twice."@en ; rdfs:isDefinedBy <urn:voc:> .
voc:note a owl:AnnotationProperty ; rdfs:label "note"@en ; dct:description "A local note."@en ; rdfs:isDefinedBy <urn:voc:> .
dct:description a owl:AnnotationProperty .
voc:Statuses a skos:ConceptScheme ; rdfs:label "Statuses"@en ; dct:description "Statuses."@en ; rdfs:isDefinedBy <urn:voc:> .
voc:Active a skos:Concept ; rdfs:label "Active"@en ; dct:description "Active."@en ; skos:inScheme voc:Statuses ; rdfs:isDefinedBy <urn:voc:> .
voc:Loose a skos:Concept ; rdfs:label "Loose"@en ; dct:description "Without a scheme."@en ; rdfs:isDefinedBy <urn:voc:> .
"#;

fn section(key: &str, term_type: &str, order: u32, extra: &str) -> String {
    format!(
        "t:{key} a ga:ReferenceSection ; ga:termType {term_type} ; ga:order \
         {order} ; ga:component t:{key}-component {extra} \
         .\nt:{key}-component a ga:Component ; dct:title \"{key}\" .\n"
    )
}

fn definitions(sections: &[String]) -> String {
    let names = sections
        .iter()
        .map(|section| section.split_whitespace().next().unwrap().to_owned())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "t:project ga:documentor t:reference .\nt:scope a ga:Scope \
         .\nt:reference a ga:DocumentorUse ; ga:documentorType \
         ga:VocabularyReferenceDocumentor ; ga:subject <urn:voc:> ; \
         ga:includedIn t:scope ; ga:referenceSection {names} .\n{}",
        sections.concat()
    )
}

fn standard_sections() -> Vec<String> {
    vec![
        section("classes", "owl:Class", 1, ""),
        section(
            "annotations",
            "owl:AnnotationProperty",
            4,
            "; ga:definedTermsOnly true",
        ),
        section("individuals", "owl:NamedIndividual", 5, ""),
        section(
            "concepts",
            "skos:Concept",
            6,
            "; ga:definedTermsOnly true",
        ),
        section("schemes", "skos:ConceptScheme", 7, ""),
        section("shapes", "sh:NodeShape", 8, ""),
    ]
}

async fn run(definitions: &str) -> Result<(Dataset, Result<()>)> {
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let directory = tempfile::tempdir()?;
    for (processor, body) in [
        ("urn:test:definitions", definitions),
        ("urn:test:content", CONTENT),
    ] {
        let path = directory.path().join(format!(
            "{}.ttl",
            processor.rsplit(':').next().unwrap()
        ));
        std::fs::write(&path, format!("{PREFIXES}{body}"))?;
        RdfFile::new(PROJECT, processor, path)?
            .collect(&writer)
            .await?;
    }
    let view = RevisionView::new(
        dataset.clone(),
        dataset
            .snapshot_for_processor(PROJECT, "urn:test:definitions")
            .await?,
    );
    let documentor = VocabularyReferenceDocumentor::new(
        dataset.clone(),
        view,
        &NamedNode::new(PROJECT)?,
        NamedNode::new("urn:test:reference")?,
    )
    .await?;
    let result = documentor.document(&writer).await;
    drop(writer);
    worker.await??;
    Ok((dataset, result))
}

/// The ordered `(fragment order, documented term)` pairs of one component.
async fn component(dataset: &Dataset, key: &str) -> Result<Vec<(u32, String)>> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(PROJECT).await?,
    );
    let component = NamedNode::new(format!("urn:test:{key}-component"))?;
    let mut rows = view.objects(&component, "fragment").await?;
    let mut terms = Vec::new();
    while let Some(row) = rows.next().await? {
        let Term::NamedNode(fragment) = &row["value"] else {
            bail!("fragment IRI required")
        };
        let order = unsigned(
            &view.optional(fragment, "order").await?.context("order")?,
            "order",
        )?;
        terms.push((
            order,
            view.node(fragment, "subject").await?.to_string(),
        ));
    }
    terms.sort();
    Ok(terms)
}

#[tokio::test]
async fn sections_select_terms_by_precedence_ownership_and_skos_scheme()
-> Result<()> {
    let (dataset, result) = run(&definitions(&standard_sections())).await?;
    result?;
    assert_eq!(component(&dataset, "classes").await?, [
        (0, "<urn:voc:Both>".to_owned()),
        (1, "<urn:voc:Thing>".to_owned())
    ]);
    assert!(
        component(&dataset, "individuals").await?.is_empty(),
        "a multi-typed term belongs only to its lowest-ordered section"
    );
    assert_eq!(
        component(&dataset, "annotations").await?,
        [(0, "<urn:voc:note>".to_owned())],
        "imported annotation declarations are not defined terms"
    );
    assert_eq!(component(&dataset, "schemes").await?, [
        (0, "<urn:voc:Statuses>".to_owned()),
        (1, "<urn:voc:Active>".to_owned())
    ]);
    assert_eq!(component(&dataset, "concepts").await?, [(
        0,
        "<urn:voc:Loose>".to_owned()
    )]);
    assert!(
        component(&dataset, "shapes").await?.is_empty(),
        "a section without matching terms leaves its component empty"
    );
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(PROJECT).await?,
    );
    let mut included = view
        .objects(&NamedNode::new("urn:test:scope")?, "include")
        .await?;
    let mut count = 0;
    while included.next().await?.is_some() {
        count += 1;
    }
    assert_eq!(count, 6, "every documented term joins the scope");
    let pattern = view.pattern(
        "?fragment <https://ekgf.org/ontology/grapharch#subject> \
         <urn:voc:Thing> ; <https://ekgf.org/ontology/grapharch#body> ?body",
        0,
    );
    let mut rows = dataset
        .select(format!("SELECT ?body WHERE {{ {pattern} }}"))
        .await?;
    let row = rows.next().await?.context("Thing fragment")?;
    assert!(
        row["body"].to_string().contains("voc:Thing"),
        "source sh:declare prefixes name the term: {}",
        row["body"]
    );
    Ok(())
}

#[tokio::test]
async fn unassigned_concepts_need_a_concept_section_and_orders_are_distinct()
-> Result<()> {
    let mut cases = vec![(
        definitions(&[
            section("classes", "owl:Class", 1, ""),
            section("schemes", "skos:ConceptScheme", 2, ""),
            section("loose", "owl:Thing", 3, ""),
        ]),
        "no reference section has ga:termType skos:Concept",
    )];
    cases[0].0.push_str("voc:Loose a owl:Thing .\n");
    cases.push((
        definitions(&[
            section("classes", "owl:Class", 1, ""),
            section("shapes", "sh:NodeShape", 1, ""),
        ]),
        "ambiguous order 1",
    ));
    cases.push((
        definitions(&[
            section("classes", "owl:Class", 1, ""),
            section("more", "owl:Class", 2, ""),
        ]),
        "select the same ga:termType",
    ));
    for (definitions, expected) in cases {
        let (_, result) = run(&definitions).await?;
        let error = format!("{:#}", result.expect_err(&definitions));
        assert!(error.contains(expected), "{expected}: {error}");
    }
    Ok(())
}
