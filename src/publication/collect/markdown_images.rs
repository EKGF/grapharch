//! Describe image requests in source fragments, then schedule distinct image
//! inputs after the Markdown batch has completed. No source file is reopened.
use {
    super::{Collector, HttpImage, source::FileInput},
    crate::publication::{dataset::Dataset, term, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    pulldown_cmark::{Event, Parser, Tag},
    std::{
        collections::{BTreeMap, BTreeSet},
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

pub(super) fn describe(input: &FileInput, body: &str) -> Result<Vec<Quad>> {
    let mut seen = BTreeSet::new();
    let mut metadata = Vec::new();
    for event in Parser::new_ext(body, crate::publication::markdown::options())
    {
        let Event::Start(Tag::Image { dest_url, .. }) = event else {
            continue
        };
        if !seen.insert(dest_url.to_string()) {
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
        let image = HttpImage::new(input.project.as_str(), &dest_url)?;
        let reference = crate::publication::dataset::identity(
            "markdown-image-reference",
            &[input.fragment_id().as_str(), &dest_url],
        );
        metadata.extend([
            input.quad(&input.fragment_id(), "asset", image.id()),
            input.quad(
                &input.fragment_id(),
                "reference",
                reference.clone(),
            ),
            input.quad(&reference, "type", term("ExternalReference")),
            input.quad(&reference, "asset", image.id()),
            input.quad(
                &reference,
                "url",
                Literal::new_simple_literal(dest_url.as_ref()),
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
