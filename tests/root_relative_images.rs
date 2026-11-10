use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, MarkdownFile},
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
        writer::GraphWriter,
    },
};
#[tokio::test]
async fn site_root_image_paths_are_retained_independently_of_page_depth()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    tokio::fs::create_dir_all(source.path().join("assets")).await?;
    tokio::fs::create_dir_all(source.path().join("nested")).await?;
    tokio::fs::write(source.path().join("assets/icon.svg"),r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><circle cx="10" cy="10" r="8"/></svg>"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "# Home\n\n![Icon](/assets/icon.svg)",
    )
    .await?;
    tokio::fs::write(
        source.path().join("nested/page.md"),
        "# Page\n\n![Icon](/assets/icon.svg)",
    )
    .await?;
    let config = source.path().join("project.ttl");
    tokio::fs::write(&config,r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>. <urn:t> a ga:Target;ga:scope <urn:s>;ga:profile <urn:f>;ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site". <urn:s> a ga:Scope. <urn:f> a ga:Profile."#).await?;
    let built =
        website::collect(source.path(), &config, "urn:p", "urn:t", 2).await?;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(result) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("failed")
    };
    let home =
        tokio::fs::read_to_string(result.directory.join("index.html")).await?;
    let nested = tokio::fs::read_to_string(
        result.directory.join("nested/page/index.html"),
    )
    .await?;
    let image = regex::Regex::new(r#"<img src="([^"]+)" alt="Icon""#)?;
    let home_image = image.captures(&home).expect(&home)[1].to_string();
    let nested_image = image.captures(&nested).expect(&nested)[1].to_string();
    assert_eq!(nested_image, format!("../../{home_image}"));
    assert!(result.directory.join(home_image).is_file());
    Ok(())
}

#[tokio::test]
async fn root_relative_images_require_an_authorized_root_and_cannot_escape()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let site = root.path().join("site");
    tokio::fs::create_dir(&site).await?;
    tokio::fs::write(
        root.path().join("outside.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#,
    )
    .await?;
    let source = site.join("index.md");
    for (destination, explicit) in [
        ("/outside.svg", false),
        ("/../outside.svg", true),
        ("//example.org/outside.svg", true),
    ] {
        tokio::fs::write(
            &source,
            format!("---\ntitle: Safety\n---\n![Image]({destination})"),
        )
        .await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) = GraphWriter::spawn(dataset, 4, 1024 * 1024)?;
        let collector = if explicit {
            MarkdownFile::in_site(
                "urn:p",
                "urn:c",
                site.clone(),
                source.clone(),
            )?
        } else {
            MarkdownFile::new("urn:p", "urn:c", source.clone())?
        };
        assert!(collector.collect(&writer).await.is_err());
        drop(writer);
        worker.await??;
    }
    Ok(())
}
