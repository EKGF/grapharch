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
async fn material_blocks_survive_restore_and_publish_semantic_html()
-> Result<()> {
    let input = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let settings = input.path().join("project.ttl");
    tokio::fs::write(&settings,r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#>.
@prefix dct:<http://purl.org/dc/terms/>.
<urn:p> a ga:Project;ga:target <urn:t>,<urn:pdf>,<urn:partial>.
<urn:s> a ga:Scope. <urn:f> a ga:Profile.
<urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site".
<urn:ps> a ga:Scope;ga:include <urn:page>;ga:traverse ga:topic;ga:maxDepth 1.
<urn:pf> a ga:Profile;ga:placement <urn:placement>.
<urn:placement> a ga:Placement;ga:component <urn:component>;ga:order 1.
<urn:component> a ga:Component;dct:title "Technical detail";ga:fragment <urn:technical>.
<urn:partial> a ga:Target;ga:scope <urn:ps>;ga:profile <urn:pf>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "partial".
<urn:grapharch:example-authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en ; <https://ekgf.org/ontology/grapharch#slug> "example" . <urn:pdf> a ga:Target;dct:title "Semantic content";ga:scope <urn:ps>;ga:profile <urn:pf>;ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "pdf" ; <https://ekgf.org/ontology/grapharch#publisher> <urn:grapharch:example-authority>;ga:destination "pdf".
"#).await?;
    tokio::fs::write(
        input.path().join("index.md"),
        r#"---
title: Panels
permalink: /
grapharch:
  subject: urn:page
  fragments:
    technical-details: urn:technical
---
# Panels

=== "Business & management"

    A **shared** [detail](detail.md).

    !!! info "Read this first"

        Nested information.

=== "Technical"

    ## Technical details

    ???+ warning "Review <carefully>"

        Visible technical detail.

    <div class="grid cards" markdown>

    - <span class="objective-badge" data-letter="1"></span> **Definition**

        First card explanation.

    - **Differences**

        Second card explanation.

    </div>

    | Identifier | Description |
    | --- | --- |
    | <span class="objective-badge objective-badge-sm" data-letter="2"></span> | Table badge |

    ```text
    === "literal, not a tab"
    ```
"#,
    )
    .await?;
    tokio::fs::write(
        input.path().join("detail.md"),
        "---\ntitle: Detail\n---\n# Detail\n",
    )
    .await?;
    let built =
        website::collect(input.path(), &settings, "urn:p", "urn:t", 2).await?;
    let (writer, worker) =
        GraphWriter::spawn(built.dataset.clone(), 4, 1024 * 1024)?;
    let revision = Assembler::new(built.dataset.clone())
        .assemble("urn:p")
        .await?;
    let pdf = TargetPlanner::new(built.dataset.clone(), revision.clone())
        .resolve("urn:p", &NamedNode::new("urn:pdf")?, &writer)
        .await?;
    let partial = TargetPlanner::new(built.dataset.clone(), revision)
        .resolve("urn:p", &NamedNode::new("urn:partial")?, &writer)
        .await?;
    drop(writer);
    worker.await??;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(input.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(result) = Publisher::new(restored.clone())
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("publish failed")
    };
    let html =
        tokio::fs::read_to_string(result.directory.join("index.html")).await?;
    assert_eq!(
        html.matches("class=\"ga-tabs\"").count(),
        1,
        "a page must retain one original tab group"
    );
    assert!(html.contains("Read this first"));
    assert!(html.contains("class=\"ga-card-group\""));
    assert!(html.contains("First card explanation."));
    assert_eq!(
        html.matches("class=\"identifier-badge\"").count(),
        2
    );
    assert!(html.contains("<details"));
    assert!(html.contains("href=\"detail/\""), "{html}");
    assert!(html.contains("literal, not a tab"));
    assert!(!html.contains("grapharch-content"));
    assert!(html.contains("Review &lt;carefully&gt;"));
    let PublishOutcome::Published(pdf_result) =
        Publisher::new(restored.clone())
            .with_edition(grapharch::publication::pdf_name::Edition::Default)
            .render(&pdf, output.path())
            .await?
    else {
        panic!("PDF publish failed")
    };
    let typst = tokio::fs::read_to_string(pdf_result.typst_source()).await?;
    assert!(typst.contains("Visible technical detail"));
    assert!(typst.contains("Second card explanation."));
    assert!(!typst.contains("Nested information"));
    assert!(!typst.contains("grapharch-content"));
    let PublishOutcome::Published(partial_result) =
        Publisher::new(restored.clone())
            .with_edition(grapharch::publication::pdf_name::Edition::Default)
            .render(&partial, output.path())
            .await?
    else {
        panic!("partial website failed")
    };
    let partial_html =
        tokio::fs::read_to_string(partial_result.directory.join("index.html"))
            .await?;
    assert!(partial_html.contains("Visible technical detail"));
    assert!(!partial_html.contains("Nested information"));
    if std::env::var_os("GRAPHARCH_MATERIAL_PREVIEW").is_some() {
        for plan in [&built.plan, &pdf] {
            Publisher::new(restored.clone())
                .with_edition(
                    grapharch::publication::pdf_name::Edition::Default,
                )
                .with_edition(
                    grapharch::publication::pdf_name::Edition::Default,
                )
                .render(
                    plan,
                    std::path::Path::new("output/material-blocks"),
                )
                .await?;
        }
    }
    Ok(())
}
