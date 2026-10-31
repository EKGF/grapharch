use {
    anyhow::Result,
    base64::{Engine, engine::general_purpose::STANDARD},
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        showcases,
    },
    sha2::{Digest, Sha256},
};

#[tokio::test]
async fn product_logo_is_selected_by_target_and_survives_source_deletion()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        20,
        20,
        image::Rgb([20, 90, 150]),
    ));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Jpeg)?;
    let bytes = bytes.into_inner();
    let fixture = format!(
        r#"{}
ex:target ga:productLogo ex:product-logo .
ex:product-logo a ga:BinaryAsset ; dct:title "Product & identity" ; ga:route "assets/product.jpg" ; ga:format "image/jpeg" ; ga:contentSize {} ; ga:contentDigest "{}" ; ga:chunk ex:logo-chunk .
ex:logo-chunk ga:order 0 ; ga:base64 "{}" .
"#,
        include_str!("../examples/specification/project.ttl"),
        bytes.len(),
        hex::encode(Sha256::digest(&bytes)),
        STANDARD.encode(&bytes)
    );
    let path = source.path().join("project.ttl");
    tokio::fs::write(&path, fixture).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    assert!(
        html.contains(
            "class=\"product-logo\" src=\"assets/product.jpg\" alt=\"Product \
             &amp; identity\""
        ),
        "product identity must render separately from authority branding"
    );
    assert_eq!(
        tokio::fs::read(publication.directory.join("assets/product.jpg"))
            .await?,
        bytes
    );
    assert!(html.contains("class=\"authority-logo\""));
    if let Ok(destination) = std::env::var("GRAPHARCH_LOGO_REVIEW") {
        let destination = std::path::Path::new(&destination);
        tokio::fs::create_dir_all(destination.join("assets")).await?;
        for name in ["index.html", "style.css", "assets/product.jpg"] {
            tokio::fs::copy(
                publication.directory.join(name),
                destination.join(name),
            )
            .await?;
        }
    }

    Ok(())
}

#[tokio::test]
async fn explicit_git_image_collection_retains_pinned_jpeg_bytes_and_provenance()
-> Result<()> {
    use grapharch::publication::{
        collect::git::remote_repository,
        writer::GraphWriter,
    };
    let source = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        10,
        10,
        image::Rgb([10, 60, 110]),
    ));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Jpeg)?;
    let bytes = bytes.into_inner();
    tokio::fs::write(source.path().join("logo.jpg"), &bytes).await?;
    tokio::fs::write(
        source.path().join("ontology.ttl"),
        "<urn:logo:ontology> a <http://www.w3.org/2002/07/owl#Ontology> .",
    )
    .await?;
    for args in [
        vec!["init", "-b", "develop"],
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@example.org"],
        vec!["add", "."],
        vec!["commit", "-m", "fixture"],
    ] {
        let status = tokio::process::Command::new("git")
            .arg("-C")
            .arg(source.path())
            .args(args)
            .output()
            .await?;
        anyhow::ensure!(status.status.success(), "fixture git failed");
    }
    let scan = remote_repository(
        source.path().to_str().unwrap(),
        "develop",
        cache.path(),
        "urn:logo:project",
    )
    .await?;
    let collector = scan
        .blob(std::path::Path::new("logo.jpg"))
        .await?
        .image_collector(
            "urn:logo:collector",
            oxrdf::NamedNode::new("urn:logo:asset")?,
        )?;
    source.close()?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 4, 4 * 1024 * 1024)?;
    collector.collect(&writer).await?;
    drop(writer);
    worker.await??;
    let view = grapharch::publication::view::RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:logo:project").await?,
    );
    let asset = oxrdf::NamedNode::new("urn:logo:asset")?;
    assert!(view.has_type(&asset, "BinaryAsset").await?);
    assert_eq!(
        view.text(&asset, "contentDigest").await?,
        hex::encode(Sha256::digest(&bytes))
    );
    assert_eq!(view.text(&asset, "format").await?, "image/jpeg");
    Ok(())
}

#[tokio::test]
async fn product_logo_rejects_literal_and_unretained_references() -> Result<()>
{
    for reference in ["\"https://example.org/logo.jpg\"", "<urn:missing:asset>"]
    {
        let source = tempfile::tempdir()?;
        let path = source.path().join("project.ttl");
        tokio::fs::write(
            &path,
            format!(
                "{}\nex:target ga:productLogo {reference} .\n",
                include_str!("../examples/specification/project.ttl")
            ),
        )
        .await?;
        let error = showcases::specification(&path)
            .await
            .err()
            .expect("invalid product logo must fail planning");
        assert!(format!("{error:#}").contains("productLogo"));
    }
    Ok(())
}

#[tokio::test]
async fn product_logo_requires_accessible_identity() -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let image = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#;
    tokio::fs::write(&path, format!(r#"{}
ex:target ga:productLogo ex:logo .
ex:logo a ga:BinaryAsset ; ga:format "image/svg+xml" ; ga:route "assets/logo.svg" ; ga:contentSize {} ; ga:contentDigest "{}" ; ga:chunk ex:logo-chunk .
ex:logo-chunk ga:order 0 ; ga:base64 "{}" .
"#, include_str!("../examples/specification/project.ttl"), image.len(), hex::encode(Sha256::digest(image)), STANDARD.encode(image))).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let output = tempfile::tempdir()?;
    let error = Publisher::new(dataset)
        .render(&plans[0], output.path())
        .await
        .err()
        .expect("missing logo alternative text must fail rendering");
    assert!(
        format!("{error:#}").contains("dcterms:title"),
        "{error:#}"
    );
    Ok(())
}
