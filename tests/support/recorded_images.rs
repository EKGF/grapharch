//! Explicit recorded image source used by offline publication acceptance tests.
use {
    anyhow::Result,
    base64::{Engine, engine::general_purpose::STANDARD},
    grapharch::publication::{
        collect::{Collector, HttpImageRequest, RdfFile},
        website::{self, CollectedWebsite},
    },
    oxrdf::Literal,
    sha2::{Digest, Sha256},
    std::{path::Path, sync::Arc},
};

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="112" height="22"><rect width="112" height="22" fill="#116659"/><text x="5" y="16" fill="white" font-size="12">Recorded image</text></svg>"##;

pub fn collector(
    root: &Path,
    project: &str,
    request: &HttpImageRequest,
) -> Result<Arc<dyn Collector>> {
    let name = hex::encode(Sha256::digest(request.url.as_bytes()));
    let processor = format!("urn:test:recorded-image:{name}");
    let source = root.join(format!("recorded-image-{name}.ttl"));
    let url = Literal::new_simple_literal(&request.url);
    let provenance = Literal::new_simple_literal(
        "Offline test fixture bytes; no HTTP response or network observation \
         is claimed.",
    );
    let path = Literal::new_simple_literal(source.to_string_lossy());
    std::fs::write(
        &source,
        format!(
            r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> .
<{asset}> a ga:BinaryAsset ; ga:route "assets/recorded-image-{name}.svg" ; ga:sourcePath {url} ; ga:format "image/svg+xml" ; ga:contentDigest "{digest}" ; ga:contentSize "{size}" ; ga:chunk <urn:test:image-chunk:{name}> ; <http://purl.org/dc/terms/provenance> {provenance} ; ga:contribution <urn:test:image-source:{name}> .
<urn:test:image-chunk:{name}> ga:order 0 ; ga:base64 "{data}" .
<{processor}> a ga:Processor .
<urn:test:image-source:{name}> a ga:Contribution ; ga:project <{project}> ; ga:processor <{processor}> ; ga:source {path} ; ga:revision "recorded-image-fixture-v1:{digest}" .
"#,
            asset = request.asset.as_str(),
            digest = hex::encode(Sha256::digest(SVG.as_bytes())),
            size = SVG.len(),
            data = STANDARD.encode(SVG.as_bytes())
        ),
    )?;
    Ok(Arc::new(RdfFile::new(
        project, &processor, source,
    )?))
}

pub async fn collect_site(
    source: &Path,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedWebsite> {
    let recorded = tempfile::tempdir()?;
    website::collect_with_asset_factory(
        source,
        definitions,
        project,
        target,
        concurrency,
        &|project, request| collector(recorded.path(), project, request),
    )
    .await
}
