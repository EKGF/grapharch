//! Photographic assets stay bounded while surviving a source-free KG restore.
use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, LocalImageFile},
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
        writer::GraphWriter,
    },
    image::ImageEncoder,
};

#[tokio::test]
async fn photographic_size_png_survives_chunked_collection_and_restored_render()
-> Result<()> {
    // bounded: one 1024x1024 RGB photograph-sized fixture (3 MiB decoded).
    let mut pixels = vec![0; 1024 * 1024 * 3];
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
        1024,
        1024,
        image::ExtendedColorType::Rgb8,
    )?;
    assert!(png.len() > 3 * 1024 * 1024);
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
<urn:photo:project> a ga:Project; ga:target <urn:photo:site> .
<urn:photo:scope> a ga:Scope . <urn:photo:profile> a ga:Profile; ga:style "reader" .
<urn:photo:site> a ga:Target; ga:scope <urn:photo:scope>; ga:profile <urn:photo:profile>; ga:renderer "website"; ga:destination "site" ."#).await?;
    let collected = website::collect(
        source.path(),
        &definitions,
        "urn:photo:project",
        "urn:photo:site",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    collected.dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&collected.plan, output.path())
        .await?
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
