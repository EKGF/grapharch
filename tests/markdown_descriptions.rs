#[path = "support/definitions.rs"]
mod definitions_fixture;

use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        ontology_family,
        render::{PublishOutcome, Publisher},
    },
    oxrdf::Literal,
    std::path::Path,
};

const DEFINITIONS: &str = r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix dct:<http://purl.org/dc/terms/> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:test:project> a ga:Project ; ga:target <urn:test:target> .
<urn:test:scope> a ga:Scope . <urn:test:profile> a ga:Profile ; pub:publishesFamily <urn:test:family> .
<urn:test:family> a gov:Family ; rdfs:label "Example family"@en .
<urn:test:reference> a gov:Reference ; gov:inFamily <urn:test:family> ; gov:refersTo <https://example.org/vocab/> .
<https://example.org/vocab/> sh:declare [sh:prefix "ex" ; sh:namespace "https://example.org/vocab/"] .
<urn:test:authority> a ga:Authority ; rdfs:label "Example Standards Council" ; ga:shortName "EX" .
<urn:test:target> a ga:Target ; dct:title "Rich ontology reference" ; dct:description "A Markdown annotation test." ; rdfs:comment "Test preview." ;
 ga:scope <urn:test:scope> ; ga:profile <urn:test:profile> ; ga:renderer "specification" ; ga:destination "specification" ;
 ga:publisher <urn:test:authority> ; ga:publicationStatus "Draft" ; ga:publicationDate "2026-10-25"^^<http://www.w3.org/2001/XMLSchema#date> ; ga:thisVersion <https://example.org/spec/> ; ga:latestVersion <https://example.org/spec/> ."#;

fn sources(root: &Path, rich: &str, extra: &str) -> Result<()> {
    std::fs::create_dir(root.join("sources"))?;
    let value = Literal::new_language_tagged_literal(rich, "en")?;
    let identical = Literal::new_simple_literal(rich);
    std::fs::write(root.join("project.ttl"), DEFINITIONS)?;
    std::fs::write(
        root.join("sources/vocab.ttl"),
        format!(
            r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix ann:<https://ekgf.org/ontology/annotation#> . @prefix ex:<https://example.org/vocab/> .
ex: a owl:Ontology ; rdfs:label "Example ontology"@en ; rdfs:comment "Short ontology summary."@en ; ann:markdownDescription {value} .
ex:Class a owl:Class ; rdfs:label "A rich class"@en ; rdfs:comment "Short class definition."@en ; ann:markdownDescription {value}, {identical} ; ann:markdownDescription "Texte français non retenu."@fr .
ex:property a owl:ObjectProperty ; rdfs:label "A rich property"@en ; ann:markdownDescription {value} .
ex:individual a owl:NamedIndividual ; rdfs:label "A rich individual"@en ; ann:markdownDescription {value} .
ex:annotation a owl:AnnotationProperty ; rdfs:label "A rich annotation"@en ; ann:markdownDescription {value} .
ex:untagged a owl:DatatypeProperty ; rdfs:label "An untagged description"@en ; ann:markdownDescription "An untagged **rich** description." .
ex:foreign a owl:Class ; rdfs:label "Foreign-only rich text"@en ; ann:markdownDescription "Ne pas publier ce texte."@fr ; rdfs:comment "English plain description remains available."@en .
ex:plain a owl:DatatypeProperty ; rdfs:label "A plain property"@en ; rdfs:comment "Plain source description."@en .
{extra}
"#
        ),
    )?;
    Ok(())
}

async fn collect(root: &Path) -> Result<ontology_family::CollectedFamily> {
    ontology_family::collect(
        &root.join("sources"),
        &root.join("project.ttl"),
        "urn:test:project",
        "urn:test:target",
        2,
    )
    .await
}

#[tokio::test]
async fn markdown_annotations_render_from_restored_kg_without_duplicate_definitions()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let rich = "## Rich explanation\n\nA **source-authored** explanation with [reference](https://example.org/reference).\n\n- First item\n- Second item\n\n- [x] Reviewed\n\n| Name | Value |\n| --- | --- |\n| Example | 42 |\n\n```turtle\n# Not a heading\n<urn:example> <urn:predicate> \\\"literal\\\" .\n```\n\n### Details\n\nAn explanation remains text; it does not implement an axiom.\n";
    sources(source.path(), rich, "")?;
    let built = collect(source.path()).await?;
    let mut missing = built
        .dataset
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?d WHERE \
             { GRAPH ?g { ?d a ga:Diagnostic ; ga:code \
             \"missing-source-definition\" } }"
                .into(),
        )
        .await?;
    assert!(
        missing.next().await?.is_none(),
        "Markdown-only resources have supplied definitions"
    );
    let mut fragments = built.dataset.select("PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?body ?format ?source WHERE { GRAPH ?g { ?f ga:subject <https://example.org/vocab/Class> ; ga:body ?body ; ga:format ?format ; ga:contribution ?c . ?c ga:source ?source } }".into()).await?;
    let row = fragments
        .next()
        .await?
        .expect("source-linked term fragment");
    let body = row["body"].to_string();
    assert!(body.contains("source-authored"));
    assert!(!body.contains("Short class definition."));
    assert!(!body.contains("Texte français"));
    assert!(
        body.contains("### Rich explanation"),
        "rich headings must nest inside the resource heading"
    );
    assert!(
        body.contains("# Not a heading"),
        "fenced code must remain intact"
    );
    assert_eq!(
        row["format"],
        Literal::new_simple_literal("text/markdown").into()
    );
    assert_eq!(
        row["source"],
        Literal::new_simple_literal("https://example.org/vocab/Class").into()
    );
    let mut originals = built.dataset.select("SELECT ?value WHERE { GRAPH ?g { <https://example.org/vocab/Class> <https://ekgf.org/ontology/annotation#markdownDescription> ?value } FILTER(LANG(?value) = 'en') }".into()).await?;
    let original = originals.next().await?.expect("retained source annotation");
    assert_eq!(
        original["value"],
        Literal::new_language_tagged_literal(rich, "en")?.into()
    );
    let out = tempfile::tempdir()?;
    let archive = out.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, out.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        std::fs::read_to_string(publication.directory.join("index.html"))?;
    assert!(html.contains("<strong>source-authored</strong>"));
    assert!(html.contains("href=\"https://example.org/reference\""));
    assert!(html.contains("type=\"checkbox\""));
    assert!(html.contains("Plain source description."));
    assert!(html.contains("English plain description remains available."));
    assert!(html.contains("An untagged <strong>rich</strong> description."));
    assert!(!html.contains("Ne pas publier ce texte."));
    let doc = scraper::Html::parse_document(&html);
    assert!(
        doc.select(&scraper::Selector::parse("h4").unwrap())
            .any(|h| h.text().collect::<String>().contains("Rich explanation"))
    );
    assert!(
        doc.select(&scraper::Selector::parse("h5").unwrap())
            .any(|h| h.text().collect::<String>().contains("Details"))
    );
    let typst =
        std::fs::read_to_string(publication.directory.join("main.typ"))?;
    assert!(typst.contains("#heading(level: 3)"));
    assert!(typst.contains("#strong["));
    assert!(typst.contains("#list("));
    assert!(typst.contains("#raw(block: true, lang: \"turtle\""));
    assert!(
        std::fs::read(publication.directory.join("document.pdf"))?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn distinct_selected_markdown_annotations_are_preserved_for_review()
-> Result<()> {
    let source = tempfile::tempdir()?;
    sources(
        source.path(),
        "Rich description.",
        "ex:Class ann:markdownDescription \"Conflicting English \
         description.\"@en .",
    )?;
    let built = collect(source.path()).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(built.dataset)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(html.contains("Rich description."));
    assert!(html.contains("Conflicting English description."));
    assert!(
        publication
            .manifest
            .diagnostics
            .iter()
            .any(|d| d.code == "competing-source-annotations")
    );
    Ok(())
}

#[tokio::test]
async fn malicious_rich_html_is_retained_as_source_and_rejected_at_rendering()
-> Result<()> {
    let source = tempfile::tempdir()?;
    sources(
        source.path(),
        "## Rich explanation\n\n<script>alert('unsafe')</script>\n",
        "",
    )?;
    let built = collect(source.path()).await?;
    let out = tempfile::tempdir()?;
    let error = Publisher::new(built.dataset.clone())
        .render(&built.plan, out.path())
        .await
        .err()
        .expect("source HTML must never execute");
    assert!(format!("{error:#}").contains("raw HTML is not supported"));
    assert!(!out.path().join("specification/index.html").exists());
    let mut diagnostics = built
        .dataset
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT \
             ?revision ?message WHERE { GRAPH ?g { ?d a ga:Diagnostic ; \
             ga:code 'publication-failed' ; ga:inputRevision ?revision ; \
             ga:message ?message } }"
                .into(),
        )
        .await?;
    let diagnostic = diagnostics
        .next()
        .await?
        .expect("failed rendering must leave a pinned KG diagnostic");
    assert!(
        diagnostic["message"]
            .to_string()
            .contains("raw HTML is not supported")
    );
    assert!(matches!(
        diagnostic["revision"],
        oxrdf::Term::NamedNode(_)
    ));

    Ok(())
}

#[tokio::test]
async fn owl_documentors_prefer_markdown_over_plain_comments() -> Result<()> {
    use grapharch::publication::{
        collect::{Collector, RdfFile},
        document::{Documentor, OwlClassDocumentor},
        writer::GraphWriter,
    };
    let root = tempfile::tempdir()?;
    let path = root.path().join("ontology.ttl");
    std::fs::write(
        &path,
        r###"<urn:class> a <http://www.w3.org/2002/07/owl#Class> ; <http://www.w3.org/2000/01/rdf-schema#comment> "A short plain definition." ; <https://ekgf.org/ontology/annotation#markdownDescription> "## Rich class explanation\n\nA **source-authored** explanation."@en ."###,
    )?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new("urn:test:project", "urn:test:collector", path)?
        .collect(&writer)
        .await?;
    let input = dataset.snapshot_for("urn:test:project").await?;
    let documentor =
        OwlClassDocumentor::new(dataset.clone(), input, "urn:test:project")?;
    documentor.document(&writer).await?;
    let fragment = documentor.fragment_id(&oxrdf::NamedNode::new("urn:class")?);
    let mut rows = dataset.select(format!("SELECT ?body WHERE {{ GRAPH ?g {{ {fragment} <https://ekgf.org/ontology/grapharch#body> ?body }} }}")).await?;
    let row = rows.next().await?.expect("generated description");
    let oxrdf::Term::Literal(body) = &row["body"] else {
        panic!("description required")
    };
    assert_eq!(
        body.value(),
        "## Rich class explanation\n\nA **source-authored** explanation."
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn single_ontology_front_matter_prefers_the_rich_annotation() -> Result<()>
{
    use grapharch::publication::website;
    let source = tempfile::tempdir()?;
    std::fs::create_dir_all(source.path().join("other/ontology"))?;
    let definition = definitions_fixture::edit_definitions(
        include_str!("../docs/grapharch.ttl"),
        &["showcasesSource"],
        None,
    )?;
    std::fs::write(source.path().join("grapharch.ttl"), definition)?;
    let annotation = Literal::new_language_tagged_literal(
        "## Ontology narrative\n\nA **rich ontology explanation**.",
        "en",
    )?;
    let class = Literal::new_language_tagged_literal(
        "## Project narrative\n\nA **rich class explanation**.",
        "en",
    )?;
    std::fs::write(source.path().join("other/ontology/publication.ttl"), format!("{}\n<https://ekgf.org/ontology/grapharch> <https://ekgf.org/ontology/annotation#markdownDescription> {annotation} .\n<https://ekgf.org/ontology/grapharch#Project> <https://ekgf.org/ontology/annotation#markdownDescription> {class} .\n", include_str!("../docs/other/ontology/publication.ttl")))?;
    std::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
         urn:test:home\n---\n# Home\n",
    )?;
    let built = website::collect(
        source.path(),
        &source.path().join("grapharch.ttl"),
        "urn:grapharch:docs",
        "urn:grapharch:docs:website",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html = std::fs::read_to_string(
        publication.directory.join("spec/develop/index.html"),
    )?;
    assert!(html.contains("<strong>rich ontology explanation</strong>"));
    assert!(html.contains("<strong>rich class explanation</strong>"));
    Ok(())
}

#[tokio::test]
async fn rich_term_revisions_reuse_persisted_contributions_when_unrelated_sources_change()
-> Result<()> {
    use grapharch::publication::ontology_family::FamilySource;
    let root = tempfile::tempdir()?;
    sources(
        root.path(),
        "## Scope\n\nA **source-authored** description.",
        "",
    )?;
    let store = tempfile::tempdir()?;
    let dataset = Dataset::open(store.path().join("kg")).await?;
    let first = ontology_family::collect_from(
        dataset.clone(),
        FamilySource::Directory(&root.path().join("sources")),
        &root.path().join("project.ttl"),
        "urn:test:project",
        "urn:test:target",
        2,
    )
    .await?;
    let again = ontology_family::collect_from(
        dataset.clone(),
        FamilySource::Directory(&root.path().join("sources")),
        &root.path().join("project.ttl"),
        "urn:test:project",
        "urn:test:target",
        2,
    )
    .await?;
    assert_eq!(
        first.plan.id, again.plan.id,
        "unchanged source contributions must reuse their publication plan"
    );
    async fn revisions(dataset: &Dataset) -> Result<Vec<String>> {
        let mut rows = dataset.select("PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT DISTINCT ?revision WHERE { GRAPH ?g { ?c ga:source 'https://example.org/vocab/Class' ; ga:revision ?revision ; ga:processor ?p . ?p a ga:Documentor } } ORDER BY ?revision".into()).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(row["revision"].to_string());
        }
        Ok(result)
    }
    let before = revisions(&dataset).await?;
    assert_eq!(before.len(), 1);
    std::fs::write(
        root.path().join("sources/unrelated.ttl"),
        "<urn:unrelated> <urn:note> \"An unrelated source observation.\" .",
    )?;
    ontology_family::collect_from(
        dataset.clone(),
        FamilySource::Directory(&root.path().join("sources")),
        &root.path().join("project.ttl"),
        "urn:test:project",
        "urn:test:target",
        2,
    )
    .await?;
    assert_eq!(
        before,
        revisions(&dataset).await?,
        "unrelated source changes must not churn the term's immutable \
         contribution"
    );
    Ok(())
}

#[tokio::test]
async fn unsafe_links_and_empty_rich_annotations_are_rejected() -> Result<()> {
    let empty = tempfile::tempdir()?;
    sources(empty.path(), "", "")?;
    let error = collect(empty.path()).await.err().expect(
        "an explicit empty rich annotation must not select a plain fallback",
    );
    assert!(
        format!("{error:#}").contains("markdownDescription cannot be empty")
    );
    let unsafe_link = tempfile::tempdir()?;
    sources(
        unsafe_link.path(),
        "[Unsafe link](javascript:alert(1))",
        "",
    )?;
    let built = collect(unsafe_link.path()).await?;
    let output = tempfile::tempdir()?;
    let error = Publisher::new(built.dataset)
        .render(&built.plan, output.path())
        .await
        .err()
        .expect("unsafe rich links must fail");
    assert!(format!("{error:#}").contains("unresolved or unsupported link"));
    Ok(())
}
