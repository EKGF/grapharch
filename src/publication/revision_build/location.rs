//! Deployment location of one revision build. The trusted orchestrator
//! supplies it as a build input; it is never source content. Every selected
//! target's `ga:thisVersion` derives from it and the target's
//! `ga:destination`, so a designated branch publishes correctly wherever it is
//! deployed.
use {
    crate::publication::{
        Renderer,
        dataset::Stage,
        pdf_name::{Edition, PdfName},
        term,
        view::RevisionView,
        writer::{ContributionStream, GraphWriter},
    },
    anyhow::{Result, anyhow, bail, ensure},
    oxrdf::{GraphName, NamedNode, Quad},
    url::Url,
};

/// Engine processor owning the derived publication versions.
pub const PROCESSOR: &str = "urn:grapharch:publication-location";
const REQUIREMENT: &str = "an absolute HTTP(S) directory URL ending in /, \
                           without credentials, query or fragment";

/// The deployed URL of a revision build's output root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationUrl(Url);

impl PublicationUrl {
    /// Accept only an absolute HTTP(S) directory URL without credentials,
    /// query or fragment.
    pub fn try_new(value: &str) -> Result<Self> {
        let url = Url::parse(value).map_err(|error| {
            anyhow!("publication URL {value:?} must be {REQUIREMENT}: {error}")
        })?;
        ensure!(
            matches!(url.scheme(), "https" | "http") &&
                url.host_str().is_some() &&
                url.username().is_empty() &&
                url.password().is_none() &&
                url.query().is_none() &&
                url.fragment().is_none() &&
                url.path().ends_with('/'),
            "publication URL {value:?} must be {REQUIREMENT}"
        );
        Ok(Self(url))
    }

    /// The URL text.
    pub fn as_str(&self) -> &str { self.0.as_str() }

    /// Where a target publishes `destination` beneath this URL: the
    /// directory of an HTML entry point, or `document`, the named file a
    /// PDF-only target owns there.
    pub fn this_version(
        &self,
        destination: &str,
        document: Option<&str>,
    ) -> Result<NamedNode> {
        ensure!(
            destination
                .split('/')
                .all(|segment| !matches!(segment, "" | "." | "..")),
            "target destination {destination:?} must be a relative directory \
             without empty, dot or traversal segments"
        );
        let mut url = self.0.clone();
        url.path_segments_mut()
            .map_err(|()| anyhow!("publication URL cannot have a path"))?
            .pop_if_empty()
            .extend(destination.split('/'))
            .push(document.unwrap_or_default());
        Ok(NamedNode::new(url.as_str())?)
    }
}

impl std::str::FromStr for PublicationUrl {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> { Self::try_new(value) }
}

impl std::fmt::Display for PublicationUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Write the derived `ga:thisVersion` of every selected target. A definition
/// that declares one itself conflicts with the deployment input.
pub(super) async fn derive(
    definitions: &RevisionView,
    project: &NamedNode,
    targets: &[NamedNode],
    publication_url: &PublicationUrl,
    edition: Option<&Edition>,
    writer: &GraphWriter,
) -> Result<()> {
    let mut stream = writer
        .begin_stage(
            project.as_str(),
            PROCESSOR,
            project.as_str(),
            &format!("publication-location-v1:{publication_url}"),
            Stage::Document,
        )
        .await?;
    match write(
        definitions,
        targets,
        publication_url,
        edition,
        &mut stream,
    )
    .await
    {
        Ok(()) => stream.complete().await,
        Err(error) => {
            stream.abort().await?;
            Err(error)
        },
    }
}

async fn write(
    definitions: &RevisionView,
    targets: &[NamedNode],
    publication_url: &PublicationUrl,
    edition: Option<&Edition>,
    stream: &mut ContributionStream,
) -> Result<()> {
    for target in targets {
        if let Some(declared) =
            definitions.optional(target, "thisVersion").await?
        {
            bail!(
                "{target} declares ga:thisVersion {declared}; a revision \
                 build derives every selected target's version from its \
                 publication URL (--publication-url) and ga:destination"
            )
        }
        let destination = definitions.text(target, "destination").await?;
        let document = if definitions.renderer(target).await? == Renderer::Typst
        {
            Some(
                PdfName::read(definitions, target, edition)
                    .await?
                    .file_name(),
            )
        } else {
            None
        };
        let version =
            publication_url.this_version(&destination, document.as_deref())?;
        stream
            .write(Quad::new(
                target.clone(),
                term("thisVersion"),
                version,
                GraphName::DefaultGraph,
            ))
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
