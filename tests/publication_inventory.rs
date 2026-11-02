use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn publication_inventory_is_a_pinned_generated_page_and_route_manifest()
-> Result<()> {
    let source = tempfile::tempdir()?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Project\npermalink: /\ngrapharch:\n  subject: \
         urn:home\n---\n# Project\n\n[Specification \
         versions](spec-versions.md)\n",
    )
    .await?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions,r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix doap: <http://usefulinc.com/ns/doap#> .
@prefix dcat: <http://www.w3.org/ns/dcat#> .
<urn:project> a ga:Project; dct:title "Project"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope . <urn:profile> a ga:Profile; ga:style "ekgf-project" .
<urn:target> a ga:Target; dct:title "Project"; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
<urn:inventory-job> a ga:Documentor; ga:project <urn:project>; ga:kind "publication-inventory"; ga:subject <urn:inventory>; ga:fragment <urn:versions-page>; ga:route "spec-versions/"; ga:sourcePath "spec-versions.md"; ga:navigationOrder 6; dct:title "Specification versions" .
<urn:inventory> dct:modified "2026-11-02T15:00:00Z"; dct:hasPart <urn:main>,<urn:develop>,<urn:preview>,<urn:hidden> .
<urn:main> a doap:Version; dct:title "DPROD 1.0"; dct:identifier "main"; doap:branch "main"; dcat:accessURL <https://example.org/dprod/spec/main/>; ga:advertised true .
<urn:develop> a doap:Version; dct:title "Develop (latest)"; dct:identifier "develop"; doap:branch "develop"; dcat:accessURL <https://example.org/dprod/spec/develop/>; ga:advertised true .
<urn:preview> a doap:Version; dct:title "ballot/4"; dct:identifier "ballot-4"; doap:branch "ballot/4"; dcat:accessURL <https://preview.example.org/dprod/spec/>; ga:advertised true .
<urn:hidden> a doap:Version; dct:title "merged-branch"; dct:identifier "merged-branch"; doap:branch "merged-branch"; dcat:accessURL <https://old.example.org/dprod/spec/>; ga:advertised false .
"#).await?;
    let collected = website::collect(
        source.path(),
        &definitions,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let export = tempfile::tempdir()?;
    collected
        .dataset
        .export(export.path().join("kg.nq"))
        .await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(export.path().join("kg.nq")).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&collected.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html = tokio::fs::read_to_string(
        publication.directory.join("spec-versions/index.html"),
    )
    .await?;
    assert!(
        html.contains("DPROD 1.0") &&
            html.contains("Develop (latest)") &&
            html.contains("ballot/4")
    );
    assert!(!html.contains("merged-branch"));
    assert!(html.contains("2026-11-02T15:00:00Z"));
    let manifest = tokio::fs::read_to_string(
        publication.directory.join("publication-versions.json"),
    )
    .await?;
    assert!(
        manifest.contains("merged-branch") &&
            manifest.contains("old.example.org")
    );
    assert!(manifest.contains("\"advertised\": false"));
    Ok(())
}
