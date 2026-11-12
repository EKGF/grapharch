//! Photographic assets stay bounded while surviving a source-free KG restore.
use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, LocalImageFile},
        dataset::Dataset,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        website,
        writer::GraphWriter,
    },
    image::ImageEncoder,
};

#[tokio::test]
async fn photographic_size_png_survives_chunked_collection_and_restored_render()
-> Result<()> {
    // bounded: one 1600x1600 RGB photograph-sized fixture (under 8 MiB
    // decoded).
    let mut pixels = vec![0; 1600 * 1600 * 3];
    let mut state = 17u32;
    for byte in &mut pixels {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = state as u8;
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        &pixels,
        1600,
        1600,
        image::ExtendedColorType::Rgb8,
    )?;
    assert!(png.len() > 6 * 1024 * 1024 && png.len() <= 8 * 1024 * 1024);
    let source = tempfile::tempdir()?;
    tokio::fs::write(source.path().join("photo.png"), &png).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Photo\npermalink: /\n---\n# Photo\n\n![Photographic \
         asset](photo.png)\n",
    )
    .await?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> .
<urn:photo:project> a ga:Project; ga:target <urn:photo:site>, <urn:photo:pdf> .
<urn:photo:scope> a ga:Scope . <urn:photo:profile> a ga:Profile; ga:theme ga:ReaderTheme .
<urn:photo:site> a ga:Target; ga:scope <urn:photo:scope>; ga:profile <urn:photo:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
<urn:photo:pdf> a ga:Target; ga:scope <urn:photo:scope>; ga:profile <urn:photo:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer>; ga:destination "review" ."#).await?;
    let collected = website::collect(
        source.path(),
        &definitions,
        "urn:photo:project",
        "urn:photo:site",
        2,
    )
    .await?;
    let (writer, worker) =
        GraphWriter::spawn(collected.dataset.clone(), 4, 1024 * 1024)?;
    let revision = Assembler::new(collected.dataset.clone())
        .assemble("urn:photo:project")
        .await?;
    let pdf = TargetPlanner::new(collected.dataset.clone(), revision)
        .resolve(
            "urn:photo:project",
            &oxrdf::NamedNode::new("urn:photo:pdf")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    collected.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publisher = Publisher::new(restored);
    let PublishOutcome::Published(publication) =
        publisher.render(&collected.plan, output.path()).await?
    else {
        panic!("publication required")
    };
    let asset = publication
        .manifest
        .files
        .iter()
        .find(|name| name.ends_with(".png"))
        .expect("retained photograph");
    assert_eq!(
        tokio::fs::read(publication.directory.join(asset)).await?,
        png
    );
    assert!(matches!(
        publisher.render(&pdf, output.path()).await?,
        PublishOutcome::Published(_)
    ));
    Ok(())
}

#[tokio::test]
async fn encoded_image_limit_rejects_oversized_sources_before_retention()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("oversized.svg");
    let file = tokio::fs::File::create(&path).await?;
    file.set_len(8 * 1024 * 1024 + 1).await?;
    drop(file);
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset, 2, 1024 * 1024)?;
    let result = LocalImageFile::new("urn:photo:project", source.path(), &path)
        .await?
        .collect(&writer)
        .await;
    assert!(
        format!(
            "{:#}",
            result.expect_err("oversized source must fail")
        )
        .contains("8 MiB")
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn rejected_authored_svg_names_the_concrete_source_asset() -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("authored-export.svg");
    tokio::fs::write(
        &path,
        "<!DOCTYPE svg><svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset, 2, 1024 * 1024)?;
    let error = LocalImageFile::new("urn:photo:project", source.path(), &path)
        .await?
        .collect(&writer)
        .await
        .expect_err("unsafe SVG must fail");
    assert!(format!("{error:#}").contains("authored-export.svg"));
    drop(writer);
    worker.await??;
    Ok(())
}
