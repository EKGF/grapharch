use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        dataset::Dataset,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        website,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
};
#[tokio::test]
async fn retained_abbreviation_schemes_project_into_html_without_changing_pdf_text()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#>.
<urn:p> a ga:Project;ga:target <urn:web>,<urn:pdf>;ga:collector <urn:dictionary>.
<urn:dictionary> a ga:CollectorUse;ga:collectorType ga:RdfFileCollector;ga:source "dictionary.ttl".
<urn:web> a ga:Target;ga:scope <urn:scope>;ga:profile <urn:profile>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site".
<urn:scope> a ga:Scope. <urn:profile> a ga:Profile.
<urn:grapharch:example-authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en ; <https://ekgf.org/ontology/grapharch#slug> "example" . <urn:pdf> a ga:Target;ga:scope <urn:pdfscope>;ga:profile <urn:pdfprofile>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "pdf" ; <https://ekgf.org/ontology/grapharch#publisher> <urn:grapharch:example-authority>;ga:destination "pdf".
<urn:pdfscope> a ga:Scope;ga:include <urn:guide>.
<urn:pdfprofile> a ga:Profile;ga:placement <urn:placement>.
<urn:placement> a ga:Placement;ga:component <urn:component>;ga:order 1.
<urn:component> a ga:Component;ga:fragment <urn:guide>.
"#).await?;
    tokio::fs::write(source.path().join("dictionary.ttl"), r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#>.
@prefix skos:<http://www.w3.org/2004/02/skos/core#>.
<urn:p> ga:abbreviationScheme <urn:abbr>.
<urn:abbr> a skos:ConceptScheme.
<urn:ekg> a skos:Concept;skos:inScheme <urn:abbr>;skos:notation "EKG";skos:definition "Enterprise Knowledge Graph".
"#).await?;
    tokio::fs::write(source.path().join("index.md"), "---\ntitle: Guide\ngrapharch:\n  fragment: urn:guide\n---\n# Guide\n\nEKG and **EKG** and [EKG](https://example.org/).\n\n`EKG` and EKGs.\n\n*[EKG]: Enterprise Knowledge Graph\n").await?;
    let built =
        website::collect(source.path(), &definitions, "urn:p", "urn:web", 2)
            .await?;
    let (writer, worker) =
        GraphWriter::spawn(built.dataset.clone(), 4, 1024 * 1024)?;
    let revision = Assembler::new(built.dataset.clone())
        .assemble("urn:p")
        .await?;
    let pdf = TargetPlanner::new(built.dataset.clone(), revision)
        .resolve("urn:p", &NamedNode::new("urn:pdf")?, &writer)
        .await?;
    drop(writer);
    worker.await??;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publisher = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
    let PublishOutcome::Published(site) =
        publisher.render(&built.plan, output.path()).await?
    else {
        panic!("site required")
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert_eq!(
        html.matches("<abbr title=\"Enterprise Knowledge Graph\">")
            .count(),
        3
    );
    assert!(html.contains("<code>EKG</code>"));
    assert!(html.contains("EKGs"));
    assert!(!html.contains("*[EKG]"));
    let PublishOutcome::Published(pdf) =
        publisher.render(&pdf, output.path()).await?
    else {
        panic!("PDF required")
    };
    let typst = tokio::fs::read_to_string(pdf.typst_source()).await?;
    assert!(!typst.contains("*[EKG]"));
    assert!(pdf.pdf().expect("published PDF edition").exists());
    Ok(())
}
