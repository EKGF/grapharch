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

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="140" height="24" viewBox="0 0 140 24"><rect width="140" height="24" rx="3" fill="#116659"/><text x="8" y="17" font-family="Liberation Sans" font-size="13" fill="white">Verified KG</text></svg>"##;

fn fixture(image: &[u8], mime: &str, route: &str, alt: &str) -> String {
    let source = include_str!("../examples/specification/project.ttl").replace(
        "A publication is a view",
        &format!(
            "![{alt}](https://example.org/badge.svg)\n\n[Repository \
             file](README.md)\n\nA publication is a view"
        ),
    );
    format!(
        r#"{source}
ex:scope ga:include ex:image .
ex:intro-body ga:asset ex:image ; ga:reference ex:image-reference, ex:source-reference .
ex:image-reference a ga:ExternalReference ; dct:identifier "https://example.org/badge.svg" ; ga:url "https://example.org/badge.svg" ; ga:asset ex:image .
ex:source-reference a ga:ExternalReference ; dct:identifier "README.md" ; ga:url "https://github.com/example/repository/blob/0123456789abcdef/README.md" .
ex:image a ga:BinaryAsset ; ga:route "{route}" ; ga:sourcePath "https://example.org/badge.svg" ; ga:format "{mime}" ; ga:contentSize "{size}" ; ga:contentDigest "{digest}" ; ga:chunk ex:image-chunk ; ga:contribution ex:authored .
ex:image-chunk ga:order 0 ; ga:base64 "{data}" .
"#,
        size = image.len(),
        digest = hex::encode(Sha256::digest(image)),
        data = STANDARD.encode(image)
    )
}

#[tokio::test]
async fn collected_markdown_svg_survives_export_and_source_deletion_for_spec_and_pdf()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    std::fs::write(
        &path,
        fixture(
            SVG.as_bytes(),
            "image/svg+xml",
            "assets/badge.svg",
            "Verified KG badge",
        ),
    )?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        std::fs::read_to_string(publication.directory.join("index.html"))?;
    assert!(html.contains("src=\"assets/badge.svg\""));
    assert!(html.contains("alt=\"Verified KG badge\""));
    assert!(html.contains("href=\"https://github.com/example/repository/blob/0123456789abcdef/README.md\""));
    let typst =
        std::fs::read_to_string(publication.directory.join("main.typ"))?;
    assert!(typst.contains("#image(\"assets/badge.svg\""));
    assert!(
        !typst.contains("#text(\"Verified KG badge\")"),
        "alt text must not leak as ordinary body text"
    );
    assert_eq!(
        std::fs::read(publication.directory.join("assets/badge.svg"))?,
        SVG.as_bytes()
    );
    assert!(
        publication
            .manifest
            .files
            .contains(&"assets/badge.svg".into())
    );
    assert!(
        std::fs::read(publication.directory.join("document.pdf"))?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn restored_image_assets_reject_unsafe_svg_and_unsafe_routes()
-> Result<()> {
    for (bytes, route) in [(r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#.as_bytes(), "assets/unsafe.svg"), (SVG.as_bytes(), "../outside.svg")] {
        let source = tempfile::tempdir()?;
        let path = source.path().join("project.ttl");
        std::fs::write(&path, fixture(bytes, "image/svg+xml", route, "Badge"))?;
        let (dataset, plans) = showcases::specification(&path).await?;
        let output = tempfile::tempdir()?;
        let error = Publisher::new(dataset).render(&plans[0], output.path()).await.err().expect("unsafe retained image must fail rendering");
        let message = format!("{error:#}");
        assert!(message.contains("SVG") || message.contains("image asset route"), "unexpected image rejection: {message}");
        assert!(!output.path().join("specification/index.html").exists());
    }
    Ok(())
}

#[tokio::test]
async fn png_and_jpeg_assets_render_without_original_files() -> Result<()> {
    for (format, mime, route) in [
        (
            image::ImageFormat::Png,
            "image/png",
            "assets/badge.png",
        ),
        (
            image::ImageFormat::Jpeg,
            "image/jpeg",
            "assets/badge.jpg",
        ),
    ] {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            160,
            32,
            image::Rgb([17, 102, 89]),
        ))
        .write_to(&mut bytes, format)?;
        let bytes = bytes.into_inner();
        let source = tempfile::tempdir()?;
        let path = source.path().join("project.ttl");
        std::fs::write(
            &path,
            fixture(&bytes, mime, route, "Retained raster image"),
        )?;
        let (dataset, plans) = showcases::specification(&path).await?;
        let output = tempfile::tempdir()?;
        let archive = output.path().join("documentation.nq");
        dataset.export(archive.clone()).await?;
        std::fs::remove_dir_all(source.path())?;
        let restored = Dataset::memory()?;
        restored.import(archive).await?;
        let PublishOutcome::Published(publication) = Publisher::new(restored)
            .render(&plans[0], output.path())
            .await?
        else {
            panic!("publication required")
        };
        assert_eq!(
            std::fs::read(publication.directory.join(route))?,
            bytes
        );
        assert!(
            std::fs::read_to_string(publication.directory.join("main.typ"))?
                .contains(&format!("#image(\"{route}\""))
        );
    }
    Ok(())
}

#[tokio::test]
async fn image_digest_alt_text_and_url_are_checked() -> Result<()> {
    let valid = fixture(
        SVG.as_bytes(),
        "image/svg+xml",
        "assets/badge.svg",
        "Badge",
    );
    for (source, expected) in [
        (
            valid.replace(
                &hex::encode(Sha256::digest(SVG.as_bytes())),
                &"0".repeat(64),
            ),
            "digest mismatch",
        ),
        (
            valid.replace("![Badge]", "![]"),
            "alternative text",
        ),
        (
            valid.replace(
                "https://example.org/badge.svg",
                "javascript:alert(1)",
            ),
            "unsafe collected reference URL",
        ),
    ] {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("project.ttl");
        std::fs::write(&path, source)?;
        let (dataset, plans) = showcases::specification(&path).await?;
        let output = tempfile::tempdir()?;
        let error = Publisher::new(dataset.clone())
            .render(&plans[0], output.path())
            .await
            .err()
            .expect("invalid image metadata must fail");
        assert!(
            format!("{error:#}").contains(expected),
            "{error:#}"
        );
        assert!(!output.path().join("specification/index.html").exists());
        let mut diagnostics = dataset
            .select(
                "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT \
                 ?revision WHERE {GRAPH ?g {?d a ga:Diagnostic ; ga:code \
                 'publication-failed' ; ga:inputRevision ?revision}}"
                    .into(),
            )
            .await?;
        assert!(
            diagnostics.next().await?.is_some(),
            "invalid image metadata must leave a pinned KG diagnostic"
        );
    }
    Ok(())
}

#[tokio::test]
async fn image_metadata_is_inherited_from_the_retained_source_fragment()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = fixture(
        SVG.as_bytes(),
        "image/svg+xml",
        "assets/badge.svg",
        "Inherited source badge",
    )
    .replace(
        "ex:intro-body ga:asset ex:image ; ga:reference",
        "ex:intro-body ga:derivedFrom ex:source-fragment \
         .\nex:source-fragment ga:asset ex:image ; ga:reference",
    );
    std::fs::write(&path, fixture)?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    assert!(
        std::fs::read_to_string(publication.directory.join("index.html"))?
            .contains("src=\"assets/badge.svg\"")
    );
    Ok(())
}

#[tokio::test]
async fn retained_image_publications_can_be_replaced_but_unowned_nested_files_are_protected()
-> Result<()> {
    for renderer in ["specification", "typst", "html"] {
        let source = tempfile::tempdir()?;
        let path = source.path().join("project.ttl");
        let content = fixture(
            SVG.as_bytes(),
            "image/svg+xml",
            "assets/badge.svg",
            "Retained badge",
        )
        .replace(
            "ga:renderer \"specification\"",
            &format!("ga:renderer \"{renderer}\""),
        );
        std::fs::write(&path, content)?;
        let (dataset, plans) = showcases::specification(&path).await?;
        let output = tempfile::tempdir()?;
        let archive = output.path().join("documentation.nq");
        dataset.export(archive.clone()).await?;
        std::fs::remove_dir_all(source.path())?;
        let restored = Dataset::memory()?;
        restored.import(archive).await?;
        let publisher = Publisher::new(restored);
        publisher.render(&plans[0], output.path()).await?;
        let PublishOutcome::Published(second) =
            publisher.render(&plans[0], output.path()).await?
        else {
            panic!("replacement required")
        };
        assert_eq!(
            std::fs::read(second.directory.join("assets/badge.svg"))?,
            SVG.as_bytes()
        );
        let guard = second.directory.join("assets/user-owned.txt");
        std::fs::write(&guard, "Preserve unrelated files.")?;
        let error = publisher
            .render(&plans[0], output.path())
            .await
            .err()
            .expect("unowned nested file must block replacement");
        assert!(format!("{error:#}").contains("unowned artifact"));
        assert_eq!(
            std::fs::read_to_string(guard)?,
            "Preserve unrelated files."
        );
    }
    Ok(())
}

#[tokio::test]
async fn unselected_fragments_do_not_leak_image_references_into_a_publication()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let selected = fixture(
        SVG.as_bytes(),
        "image/svg+xml",
        "assets/badge.svg",
        "Selected badge",
    );
    std::fs::write(
        &path,
        format!(
            r#"{selected}
ex:outside-source a ga:Fragment ; ga:subject ex:outside-subject ; ga:body "Outside this publication." ; ga:format "text/markdown" ; ga:contribution ex:authored ; ga:reference ex:outside-reference .
ex:outside-reference a ga:ExternalReference ; dct:identifier "https://outside.example/badge.svg" ; ga:url "https://outside.example/badge.svg" ; ga:asset ex:outside-asset .
"#
        ),
    )?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    assert!(publication.directory.join("assets/badge.svg").exists());
    assert!(!publication.directory.join("assets/outside.svg").exists());
    Ok(())
}

#[tokio::test]
async fn selected_image_reference_without_retained_bytes_is_still_an_error()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let selected = fixture(
        SVG.as_bytes(),
        "image/svg+xml",
        "assets/badge.svg",
        "Missing badge",
    )
    .replace(
        "ga:url \"https://example.org/badge.svg\" ; ga:asset ex:image .",
        "ga:url \"https://example.org/badge.svg\" ; ga:asset ex:missing-image \
         .",
    );
    std::fs::write(&path, selected)?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let output = tempfile::tempdir()?;
    let error = Publisher::new(dataset)
        .render(&plans[0], output.path())
        .await
        .err()
        .expect("selected missing image must not be ignored");
    assert!(format!("{error:#}").contains(
        "reference image is absent from the pinned publication assets"
    ));
    assert!(!output.path().join("specification/index.html").exists());
    Ok(())
}
