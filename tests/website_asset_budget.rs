use {
    anyhow::Result,
    base64::{Engine, engine::general_purpose::STANDARD},
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    sha2::{Digest, Sha256},
};

async fn fixture(count: usize) -> Result<tempfile::TempDir> {
    let source = tempfile::tempdir()?;
    let mut definitions = String::from(
        r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix dct:<http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Asset publication"; ga:scope <urn:scope>; ga:target <urn:site> .
<urn:scope> a ga:Scope .
<urn:profile> a ga:Profile .
<urn:site> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
"#,
    );
    // Bounded: reproduce both existing producers, up to 256 authored assets
    // plus the gallery's 512 retained publication artifacts, and one overflow.
    for index in 0..count {
        let (mime, body, route) = if index < 512 {
            (
                "text/html",
                "<!doctype html><title>Retained specification</title>",
                format!("showcases/artifacts/specification-{index}.html"),
            )
        } else {
            (
                "image/svg+xml",
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" \
                 height=\"8\"><rect width=\"8\" height=\"8\"/></svg>",
                format!("assets/image-{index}.svg"),
            )
        };
        definitions.push_str(&format!(r#"
<urn:scope> ga:include <urn:asset:{index}> .
<urn:asset:{index}> a ga:BinaryAsset; ga:route "{route}"; ga:sourcePath "{route}"; ga:format "{mime}"; ga:contentSize {size}; ga:contentDigest "{digest}"; ga:chunk <urn:chunk:{index}> .
<urn:chunk:{index}> ga:order 0; ga:base64 "{encoded}" .
"#, size=body.len(), digest=hex::encode(Sha256::digest(body)), encoded=STANDARD.encode(body)));
    }
    tokio::fs::write(source.path().join("project.ttl"), definitions).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\n---\n# Home\n\nRetained publications \
         and authored images.\n",
    )
    .await?;
    Ok(source)
}

#[tokio::test]
async fn authored_and_retained_publication_assets_share_a_restored_website()
-> Result<()> {
    let source = fixture(512 + 256).await?;
    let built = website::collect(
        source.path(),
        &source.path().join("project.ttl"),
        "urn:project",
        "urn:site",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publisher = Publisher::new(restored);
    // The same full inventory must also remain replaceable, not fail at the
    // later ownership-manifest boundary.
    for _ in 0..2 {
        let PublishOutcome::Published(publication) =
            publisher.render(&built.plan, output.path()).await?
        else {
            panic!("publication required")
        };
        for path in [
            "index.html",
            "showcases/artifacts/specification-511.html",
            "assets/image-767.svg",
        ] {
            assert!(
                tokio::fs::try_exists(publication.directory.join(path)).await?,
                "missing retained artifact {path}"
            );
        }
        assert!(publication.manifest.files.len() > 768);
    }
    Ok(())
}

#[tokio::test]
async fn website_asset_inventory_still_has_a_hard_aggregate_bound() -> Result<()>
{
    let source = fixture(512 + 256 + 1).await?;
    let error = website::collect(
        source.path(),
        &source.path().join("project.ttl"),
        "urn:project",
        "urn:site",
        2,
    )
    .await
    .err()
    .expect("oversized asset inventory must fail planning");
    assert!(
        error.to_string().contains("asset budget"),
        "{error:#}"
    );
    Ok(())
}
