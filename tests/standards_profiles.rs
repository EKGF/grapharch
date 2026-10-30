use grapharch::publication::{
    assembly::Assembler,
    collect::{Collector, RdfFile},
    dataset::Dataset,
    plan::TargetPlanner,
    writer::GraphWriter,
};

async fn resolve(
    extra: &str,
) -> anyhow::Result<anyhow::Result<grapharch::publication::plan::ResolvedPlan>>
{
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    tokio::fs::write(
        &path,
        format!(
            "{}\n{extra}",
            include_str!("fixtures/publication/projects.ttl")
        ),
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
    RdfFile::new(
        "urn:example:ontology-project",
        "urn:test:standards",
        path,
    )?
    .collect(&writer)
    .await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:example:ontology-project")
        .await?;
    let result = TargetPlanner::new(dataset, revision)
        .resolve(
            "urn:example:ontology-project",
            &oxrdf::NamedNode::new("urn:example:spec")?,
            &writer,
        )
        .await;
    drop(writer);
    worker.await??;
    Ok(result)
}

#[tokio::test]
async fn required_sections_must_be_selected_not_merely_present_in_the_kg()
-> anyhow::Result<()> {
    let result = resolve(r#"ex:spec-profile ga:requiredSectionRole "conformance" . ex:reader-intro ga:sectionRole "conformance" ."#).await?;
    let error = result.expect_err(
        "another profile's section cannot satisfy the specification",
    );
    assert!(error.to_string().contains("conformance"));
    Ok(())
}

#[tokio::test]
async fn required_release_metadata_cannot_be_missing_or_empty()
-> anyhow::Result<()> {
    for extra in [
        "ex:spec-profile ga:requiredMetadata ga:documentNumber .",
        "ex:spec-profile ga:requiredMetadata ga:documentNumber . ex:spec \
         ga:documentNumber \"  \" .",
    ] {
        assert!(
            resolve(extra).await?.is_err(),
            "release metadata must be supplied explicitly"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_configured_profile_accepts_its_selected_content_and_explicit_metadata()
-> anyhow::Result<()> {
    resolve(r#"ex:spec-profile ga:requiredSectionRole "scope" ; ga:requiredMetadata ga:documentNumber . ex:spec-intro ga:sectionRole "scope" . ex:spec ga:documentNumber "example/26-01-01" ."#).await??;
    Ok(())
}

#[tokio::test]
async fn release_statuses_are_explicit_authority_policy() -> anyhow::Result<()>
{
    let draft = resolve(r#"ex:spec-profile ga:allowedPublicationStatus "Formal Specification" . ex:spec ga:publicationStatus "Documentation Preview" ."#).await?;
    assert!(
        draft.is_err(),
        "a draft must not satisfy a formal release profile"
    );
    resolve(r#"ex:spec-profile ga:allowedPublicationStatus "Formal Specification" . ex:spec ga:publicationStatus "Formal Specification" ."#).await??;
    Ok(())
}

#[tokio::test]
async fn out_of_scope_placements_do_not_satisfy_required_roles()
-> anyhow::Result<()> {
    let result = resolve(r#"
ex:spec-profile ga:requiredSectionRole "conformance" ; ga:placement ex:hidden .
ex:hidden a ga:Placement ; ga:order 2 ; ga:component ex:hidden-component ; ga:sectionRole "conformance" .
ex:hidden-component a ga:Component ; ga:fragment ex:hidden-fragment .
ex:hidden-fragment a ga:Fragment ; ga:subject ex:internal ; ga:format "text/markdown" ; ga:body "Excluded conformance text." ; ga:contribution ex:authored .
"#).await?;
    assert!(
        result.is_err(),
        "an excluded section must not satisfy the role requirement"
    );
    Ok(())
}

#[tokio::test]
async fn release_identity_survives_source_removal_and_appears_in_html_and_pdf()
-> anyhow::Result<()> {
    use grapharch::publication::{
        render::{PublishOutcome, Publisher},
        showcases,
    };
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let text = format!(
        "{}\nex:profile ga:requiredMetadata ga:documentNumber, \
         ga:releaseVersion . ex:target ga:documentNumber \"example/26-01-01\" \
         ; ga:releaseVersion \"1.1 draft\" .",
        include_str!("../examples/specification/project.ttl")
    );
    tokio::fs::write(&path, text).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_file(path).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    for filename in ["index.html", "main.typ"] {
        let content =
            tokio::fs::read_to_string(publication.directory.join(filename))
                .await?;
        assert!(
            content.contains("example/26-01-01") &&
                content.contains("1.1 draft")
        );
    }
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}
