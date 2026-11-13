use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        dataset::Dataset,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        view::RevisionView,
        website,
        writer::GraphWriter,
    },
    oxrdf::{Literal, NamedNode},
};
fn definitions(path: &str) -> String {
    format!(
        r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>.
<urn:p> a ga:Project;ga:contentSource "docs";ga:collector <urn:dictionary>;ga:target <urn:web>,<urn:pdf>.
<urn:dictionary> a ga:CollectorUse;ga:collectorType ga:AbbreviationFileCollector;ga:source {}.
<urn:web> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site".
<urn:s> a ga:Scope. <urn:f> a ga:Profile.
<urn:grapharch:example-authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en ; <https://ekgf.org/ontology/grapharch#slug> "example" . <urn:pdf> a ga:Target;ga:scope <urn:pdfscope>;ga:profile <urn:pdfprofile>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "pdf" ; <https://ekgf.org/ontology/grapharch#publisher> <urn:grapharch:example-authority>;ga:destination "pdf".
<urn:pdfscope> a ga:Scope;ga:include <urn:guide>. <urn:pdfprofile> a ga:Profile;ga:placement <urn:placement>.
<urn:placement> a ga:Placement;ga:component <urn:component>;ga:order 1. <urn:component> a ga:Component;ga:fragment <urn:guide>."#,
        Literal::new_simple_literal(path)
    )
}
async fn fixture(body: &[u8]) -> Result<tempfile::TempDir> {
    let source = tempfile::tempdir()?;
    tokio::fs::create_dir(source.path().join("docs")).await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "---\ntitle: Guide\ngrapharch:\n  fragment: urn:guide\n---\n# \
         Guide\n\nEKG and API; `EKG` and EKGs.\n",
    )
    .await?;
    tokio::fs::write(source.path().join("abbreviations.md"), body).await?;
    tokio::fs::write(
        source.path().join("project.ttl"),
        definitions("abbreviations.md"),
    )
    .await?;
    Ok(source)
}
#[tokio::test]
async fn dictionary_source_and_skos_meanings_survive_source_deletion()
-> Result<()> {
    let original = "[//]: # (original source comment)\r\n*[EKG]: Enterprise \
                    Knowledge Graph\r\n*[API]: Application Programming \
                    Interface\r\n";
    let source = fixture(original.as_bytes()).await?;
    let output = tempfile::tempdir()?;
    let built = website::collect(
        source.path(),
        &source.path().join("project.ttl"),
        "urn:p",
        "urn:web",
        2,
    )
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
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let plan_view =
        RevisionView::new(restored.clone(), built.plan.snapshot.clone());
    let source_revision =
        plan_view.node(&built.plan.id, "inputRevision").await?;
    let view = RevisionView::new(restored.clone(), source_revision.clone());
    let scheme = view
        .node(&NamedNode::new("urn:p")?, "abbreviationScheme")
        .await?;
    let original_fragment = view.node(&scheme, "derivedFrom").await?;
    assert_eq!(
        view.text(&original_fragment, "body").await?,
        original
    );
    assert_eq!(
        view.text(&original_fragment, "format").await?,
        "text/plain"
    );
    let pattern = Dataset::logical_pattern(&source_revision, &format!("?concept a <http://www.w3.org/2004/02/skos/core#Concept>; <http://www.w3.org/2004/02/skos/core#inScheme> {scheme}; <http://www.w3.org/2004/02/skos/core#notation> ?term; <http://www.w3.org/2004/02/skos/core#definition> ?definition"), 0);
    let mut terms = restored
        .select(format!(
            "SELECT ?term ?definition WHERE {{ {pattern} }}"
        ))
        .await?;
    let mut count = 0;
    while terms.next().await?.is_some() {
        count += 1;
    }
    assert_eq!(count, 2);
    let pattern = Dataset::logical_pattern(&source_revision, &format!("?fragment <https://ekgf.org/ontology/grapharch#subject> {scheme}; <https://ekgf.org/ontology/grapharch#body> ?body; <https://ekgf.org/ontology/grapharch#sourcePath> ?path"), 0);
    let mut raw = restored
        .select(format!(
            "SELECT ?body ?path WHERE {{ {pattern} }}"
        ))
        .await?;
    let row = raw.next().await?.expect("original dictionary source");
    assert_eq!(
        row["body"],
        Literal::new_simple_literal(original).into()
    );
    assert_eq!(
        row["path"],
        Literal::new_simple_literal("abbreviations.md").into()
    );
    let publisher = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
    let PublishOutcome::Published(site) =
        publisher.render(&built.plan, output.path()).await?
    else {
        panic!("site required")
    };
    let html =
        tokio::fs::read_to_string(site.directory.join("index.html")).await?;
    assert!(
        html.contains("<abbr title=\"Enterprise Knowledge Graph\">EKG</abbr>")
    );
    assert!(html.contains(
        "<abbr title=\"Application Programming Interface\">API</abbr>"
    ));
    assert!(html.contains("<code>EKG</code>"));
    assert!(html.contains("EKGs"));
    let PublishOutcome::Published(pdf) =
        publisher.render(&pdf, output.path()).await?
    else {
        panic!("PDF required")
    };
    assert!(
        tokio::fs::metadata(pdf.pdf().expect("published PDF edition"))
            .await?
            .len() >
            1000
    );
    Ok(())
}
#[tokio::test]
async fn malformed_conflicting_and_oversized_dictionaries_fail() -> Result<()> {
    let cases = [
        (b"ordinary prose".to_vec(), "line 1"),
        (vec![0xff], "UTF-8"),
        (
            b"[//]: # (comment only)\n".to_vec(),
            "no definitions",
        ),
        (
            b"*[EKG]: One\n*[EKG]: Other\n".to_vec(),
            "duplicate abbreviation",
        ),
        (
            b"*[EKG]: One\n*[EKG]: One\n".to_vec(),
            "duplicate abbreviation",
        ),
        (
            format!("*[{}]: Definition", "X".repeat(129)).into_bytes(),
            "128",
        ),
        (
            format!("*[X]: {}", "x".repeat(4097)).into_bytes(),
            "4096",
        ),
        (vec![b'x'; 128 * 1024 + 1], "128 KiB"),
        (
            (0..1025)
                .map(|index| format!("*[X{index}]: Definition\n"))
                .collect::<String>()
                .into_bytes(),
            "1024",
        ),
    ];
    for (body, expected) in cases {
        let source = fixture(&body).await?;
        let error = match website::collect(
            source.path(),
            &source.path().join("project.ttl"),
            "urn:p",
            "urn:web",
            2,
        )
        .await
        {
            Ok(_) => panic!("invalid dictionary accepted"),
            Err(error) => format!("{error:#}"),
        };
        assert!(error.contains(expected), "{error}");
        assert!(error.contains("abbreviations.md"), "{error}");
    }
    Ok(())
}
#[tokio::test]
async fn dictionary_paths_cannot_escape_the_project_root() -> Result<()> {
    let source = fixture(b"*[EKG]: Enterprise Knowledge Graph\n").await?;
    tokio::fs::write(
        source.path().join("project.ttl"),
        definitions("../abbreviations.md"),
    )
    .await?;
    let error = match website::collect(
        source.path(),
        &source.path().join("project.ttl"),
        "urn:p",
        "urn:web",
        2,
    )
    .await
    {
        Ok(_) => panic!("escaping dictionary accepted"),
        Err(error) => format!("{error:#}"),
    };
    assert!(error.contains("project-relative"), "{error}");
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn dictionary_symlinks_cannot_escape_the_project_root() -> Result<()> {
    let source = fixture(b"*[EKG]: Enterprise Knowledge Graph\n").await?;
    let outside = tempfile::tempdir()?;
    let target = outside.path().join("outside.md");
    tokio::fs::write(
        &target,
        "*[OUTSIDE]: Outside the authorized root\n",
    )
    .await?;
    tokio::fs::remove_file(source.path().join("abbreviations.md")).await?;
    tokio::fs::symlink(&target, source.path().join("abbreviations.md")).await?;
    let error = match website::collect(
        source.path(),
        &source.path().join("project.ttl"),
        "urn:p",
        "urn:web",
        2,
    )
    .await
    {
        Ok(_) => panic!("escaping dictionary symlink accepted"),
        Err(error) => format!("{error:#}"),
    };
    assert!(error.contains("escapes project root"), "{error}");
    assert!(error.contains("abbreviations.md"), "{error}");
    Ok(())
}
