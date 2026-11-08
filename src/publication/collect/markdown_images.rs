//! Describe image requests in source fragments, then schedule distinct image
//! inputs after the Markdown batch has completed. No source file is reopened.
use {
    super::{Collector, HttpImage, LocalImageFile, source::FileInput},
    crate::publication::{dataset::Dataset, term, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    pulldown_cmark::{Event, Parser, Tag},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::Path,
        sync::Arc,
    },
};

/// An HTTPS image requested by collected Markdown in a project.
#[derive(Clone, Debug)]
pub struct HttpImageRequest {
    /// Source URL. Discovery never fetches it.
    pub url:   String,
    /// Stable project-scoped identity of the retained image.
    pub asset: NamedNode,
}

/// Embedders can supply specialized image collectors, including recorded
/// sources. The default publication pipeline uses the bounded HTTPS collector.
pub type AssetCollectorFactory<'a> = dyn Fn(&str, &HttpImageRequest) -> Result<Arc<dyn Collector>>
    + Send
    + Sync
    + 'a;

pub(crate) fn default_http_image_factory(
    project: &str,
    request: &HttpImageRequest,
) -> Result<Arc<dyn Collector>> {
    Ok(Arc::new(HttpImage::new(project, &request.url)?))
}

fn generated_binding<'a>(
    root: Option<&Path>,
    source: &Path,
    destination: &str,
    bindings: Option<&'a BTreeMap<String, NamedNode>>,
) -> Result<Option<&'a NamedNode>> {
    let (Some(root), Some(bindings)) = (root, bindings) else {
        return Ok(None);
    };
    let root_url = url::Url::from_directory_path(root)
        .map_err(|()| anyhow::anyhow!("invalid image root"))?;
    // File URLs remove Windows canonical verbatim prefixes. Compare both
    // paths in that same representation without materializing generated files.
    let root = root_url
        .to_file_path()
        .map_err(|()| anyhow::anyhow!("invalid image root path"))?;
    let base = if destination.starts_with('/') {
        root_url
    } else {
        url::Url::from_file_path(source)
            .map_err(|()| anyhow::anyhow!("invalid image source"))?
    };
    let resolved = base.join(destination.trim_start_matches('/'))?;
    ensure!(
        resolved.scheme() == "file" && resolved.query().is_none(),
        "generated image requires a local path without query"
    );
    let path = resolved
        .to_file_path()
        .map_err(|()| anyhow::anyhow!("invalid generated image path"))?;
    ensure!(
        path.starts_with(&root),
        "generated image escapes documentation root"
    );
    let relative = super::page::source_path(&root, &path)?;
    Ok(bindings.get(&relative))
}

pub(super) async fn describe(
    input: &FileInput,
    root: Option<&Path>,
    body: &str,
    bindings: Option<&BTreeMap<String, NamedNode>>,
) -> Result<Vec<Quad>> {
    let mut seen = BTreeSet::new();
    let mut metadata = Vec::new();
    let mut interactive = BTreeSet::new();
    let mut schemes = BTreeMap::new();
    for event in Parser::new_ext(body, crate::publication::markdown::options())
    {
        let Event::Start(Tag::Image { dest_url, title, .. }) = event else {
            continue
        };
        if crate::publication::icon::Icon::parse_uri(&dest_url)?.is_some() {
            continue;
        }
        let first = seen.insert(dest_url.to_string());
        let hints = crate::publication::image_hints::ImageHints::parse(&title)?;
        if hints.interactive_svg && interactive.insert(dest_url.to_string()) {
            let reference = crate::publication::dataset::identity(
                "markdown-image-reference",
                &[input.fragment_id().as_str(), &dest_url],
            );
            metadata.push(input.quad(
                &reference,
                "imagePresentation",
                Literal::new_simple_literal("interactive-svg"),
            ));
        }
        if let Some(scheme) = hints.color_scheme {
            let previous = schemes.insert(dest_url.to_string(), scheme);
            ensure!(
                previous.is_none_or(|value| value == scheme),
                "one image URL has conflicting color-scheme hints"
            );
            if previous.is_none() {
                let reference = crate::publication::dataset::identity(
                    "markdown-image-reference",
                    &[input.fragment_id().as_str(), &dest_url],
                );
                let value = match scheme {
                    crate::publication::image_hints::ColorScheme::Light => {
                        "light"
                    },
                    crate::publication::image_hints::ColorScheme::Dark => {
                        "dark"
                    },
                };
                metadata.push(input.quad(
                    &reference,
                    "imageColorScheme",
                    Literal::new_simple_literal(value),
                ));
            }
        }
        if !first {
            continue;
        }
        ensure!(
            seen.len() <= 128,
            "Markdown source exceeds image request budget"
        );
        ensure!(
            dest_url.len() <= 4096,
            "Markdown image URL exceeds byte budget"
        );
        let (asset, url) = match url::Url::parse(&dest_url) {
            Ok(_) => {
                (
                    HttpImage::new(input.project.as_str(), &dest_url)?.id(),
                    dest_url.to_string(),
                )
            },
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                if let Some(asset) =
                    generated_binding(root, &input.path, &dest_url, bindings)?
                {
                    (asset.clone(), asset.as_str().to_owned())
                } else {
                    let image = LocalImageFile::reference(
                        input.project.as_str(),
                        root,
                        &input.path,
                        &dest_url,
                    )
                    .await?;
                    let asset = image.id();
                    let url = asset.as_str().to_owned();
                    (asset, url)
                }
            },
            Err(error) => return Err(error.into()),
        };
        let reference = crate::publication::dataset::identity(
            "markdown-image-reference",
            &[input.fragment_id().as_str(), &dest_url],
        );
        metadata.extend([
            input.quad(&input.fragment_id(), "asset", asset.clone()),
            input.quad(
                &input.fragment_id(),
                "reference",
                reference.clone(),
            ),
            input.quad(&reference, "type", term("ExternalReference")),
            input.quad(&reference, "asset", asset.clone()),
            input.quad(
                &reference,
                "url",
                Literal::new_simple_literal(&url),
            ),
            Quad::new(
                reference,
                NamedNode::new_unchecked("http://purl.org/dc/terms/identifier"),
                Literal::new_simple_literal(dest_url.as_ref()),
                GraphName::DefaultGraph,
            ),
        ]);
    }
    Ok(metadata)
}

/// Discover one image collector per distinct URL after source collection.
/// Already retained HTTPS images are still observed by the selected factory;
/// discovery does not impose an implicit cache or refresh policy.
pub async fn http_image_collectors(
    dataset: &Dataset,
    project: &str,
) -> Result<Vec<Arc<dyn Collector>>> {
    http_image_collectors_with_factory(
        dataset,
        project,
        &default_http_image_factory,
    )
    .await
}

/// Discover bounded, deduplicated requests and bind specialized collectors.
/// This is a collection-stage interface; renderers never use the factory.
pub async fn http_image_collectors_with_factory(
    dataset: &Dataset,
    project: &str,
    factory: &AssetCollectorFactory<'_>,
) -> Result<Vec<Arc<dyn Collector>>> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(project).await?,
    );
    let pattern = view.pattern(
        &format!(
            "?fragment a {} ; {} ?reference ; {} ?asset . ?reference {} \
             ?asset ; {} ?url",
            term("Fragment"),
            term("reference"),
            term("asset"),
            term("asset"),
            term("url")
        ),
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?asset ?url WHERE {{ {pattern} }} ORDER BY ?url \
             ?asset"
        ))
        .await?;
    let mut requests = BTreeMap::new();
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        count += 1;
        ensure!(
            count <= 4096,
            "project image discovery exceeds reference budget"
        );
        let (Term::NamedNode(asset), Term::Literal(url)) =
            (&row["asset"], &row["url"])
        else {
            anyhow::bail!(
                "image requests require named assets and literal URLs"
            )
        };
        ensure!(
            url.value().len() <= 4096 && asset.as_str().len() <= 4096,
            "image request identity exceeds byte budget"
        );
        // A repository-local image has already been collected from its Git
        // blob. Its explicit asset IRI is not an HTTP request.
        if url.value().starts_with("urn:") && url.value() == asset.as_str() {
            continue;
        }
        ensure!(
            url.value().len() <= 4096,
            "image request URL exceeds byte budget"
        );
        let image = HttpImage::new(project, url.value())?;
        ensure!(
            image.id() == *asset,
            "image request identity disagrees with its source URL"
        );
        requests.insert(url.value().to_owned(), asset.clone());
        ensure!(
            requests.len() <= 256,
            "project exceeds HTTP image request budget"
        );
    }
    let mut work = Vec::new();
    let mut identities = BTreeSet::new();
    for (url, asset) in requests {
        let collector = factory(project, &HttpImageRequest { url, asset })?;
        let identity = collector.identity()?;
        ensure!(
            identity.project == project,
            "image factory returned a collector for another project"
        );
        ensure!(
            identities.insert((
                identity.processor.to_owned(),
                identity.source.to_owned()
            )),
            "image factory returned colliding collector inputs"
        );
        work.push(collector);
    }
    Ok(work)
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn canonical_windows_roots_match_file_url_paths_without_materialized_assets()
     {
        let root = Path::new(r"\\?\C:\grapharch\docs");
        let source = root.join("concept/index.md");
        let asset = NamedNode::new("urn:diagram:asset").unwrap();
        let bindings =
            BTreeMap::from([("diagrams/example.svg".into(), asset.clone())]);
        for destination in ["../diagrams/example.svg", "/diagrams/example.svg"]
        {
            assert_eq!(
                generated_binding(
                    Some(root),
                    &source,
                    destination,
                    Some(&bindings)
                )
                .unwrap(),
                Some(&asset)
            );
        }
        assert!(
            generated_binding(
                Some(root),
                &source,
                "../../outside.svg",
                Some(&bindings)
            )
            .is_err()
        );
        assert!(
            generated_binding(
                Some(root),
                &source,
                "/../docs-sibling/outside.svg",
                Some(&bindings)
            )
            .is_err()
        );
    }

    #[test]
    fn generated_outputs_resolve_without_files_and_never_escape_the_root() {
        let root =
            std::env::temp_dir().join("grapharch-unmaterialized-image-root");
        let source = root.join("concept/index.md");
        let asset = NamedNode::new("urn:diagram:asset").unwrap();
        let bindings = BTreeMap::from([(
            "diagrams/out/example.svg".into(),
            asset.clone(),
        )]);
        for path in [
            "../diagrams/out/example.svg#darkable",
            "/diagrams/out/example.svg#darkable",
        ] {
            assert_eq!(
                generated_binding(Some(&root), &source, path, Some(&bindings))
                    .unwrap(),
                Some(&asset)
            );
        }
        assert!(
            generated_binding(
                Some(&root),
                &source,
                "../../outside.svg",
                Some(&bindings)
            )
            .is_err()
        );
        assert!(
            generated_binding(
                Some(&root),
                &source,
                "../missing.svg",
                Some(&bindings)
            )
            .unwrap()
            .is_none()
        );
    }
}
