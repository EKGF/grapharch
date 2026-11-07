use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn native_maturity_pages_combine_retained_rdf_and_authored_sections()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let out = tempfile::tempdir()?;
    tokio::fs::create_dir_all(source.path().join("docs")).await?;
    tokio::fs::create_dir_all(
        source
            .path()
            .join("fragments/pillar/business/strategy/vision"),
    )
    .await?;
    tokio::fs::write(
        source.path().join("docs/index.md"),
        "# Maturity\n\n[Explore](pillar/index.md)",
    )
    .await?;
    tokio::fs::write(
        source
            .path()
            .join("fragments/pillar/business/strategy/vision/levels.md"),
        "## Level 1\n\n- [ ] Shared goals are recorded.",
    )
    .await?;
    tokio::fs::write(source.path().join("model.ttl"), r#"
@prefix mm: <https://raw.githubusercontent.com/EKGF/ontology-maturity-model/main/maturity-model.ttl#>.
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#>.
<urn:model> a mm:Model; rdfs:label "Maturity"; mm:iriLocalName "model".
<urn:pillar> a mm:Pillar; rdfs:label "Business"; mm:iriLocalName "business"; mm:pillarInModel <urn:model>.
<urn:area> a mm:CapabilityArea; rdfs:label "Strategy"; mm:iriLocalName "strategy"; mm:inPillar <urn:pillar>.
<urn:capability> a mm:Capability; rdfs:label "Vision"; mm:iriLocalName "vision"; mm:inArea <urn:area>; mm:capabilityNumber "A.1.1".
"#).await?;
    let config = source.path().join("project.ttl");
    tokio::fs::write(&config, r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#>.
<urn:p> a ga:Project; ga:contentSource "docs"; ga:collector <urn:rdf>; ga:documentor <urn:maturity>; ga:target <urn:t>.
<urn:rdf> a ga:Collector; ga:kind "rdf-file"; ga:source "model.ttl".
<urn:maturity> a ga:Documentor; ga:kind "maturity-model"; ga:source "fragments"; ga:route "pillar/".
<urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer "website";ga:destination "site".
<urn:s> a ga:Scope. <urn:f> a ga:Profile.
"#).await?;
    let built =
        website::collect(source.path(), &config, "urn:p", "urn:t", 2).await?;
    let archive = out.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    if std::env::var_os("GRAPHARCH_MATURITY_PREVIEW").is_some() {
        tokio::fs::copy(&archive, "/tmp/maturity-website.nq").await?;
    }
    tokio::fs::remove_dir_all(source.path()).await?;
    let dataset = Dataset::memory()?;
    dataset.import(archive).await?;
    let PublishOutcome::Published(result) = Publisher::new(dataset)
        .render(&built.plan, out.path())
        .await?
    else {
        panic!("publication failed")
    };
    let model =
        tokio::fs::read_to_string(result.directory.join("pillar/index.html"))
            .await?;
    assert!(model.contains("business/"));
    let capability = tokio::fs::read_to_string(
        result
            .directory
            .join("pillar/business/strategy/vision/index.html"),
    )
    .await?;
    assert!(capability.contains("A.1.1"));
    assert!(capability.contains("Shared goals are recorded"));
    assert!(capability.contains("type=\"checkbox\""));
    assert!(!capability.contains("href=\"urn:area\""));
    assert!(
        capability.contains(
            "href=\"../../../../pillar/business/strategy/#strategy\""
        ),
        "{capability}"
    );
    Ok(())
}
