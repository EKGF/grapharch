//! RDF formats as the knowledge graph declares them. Each serialized format
//! IRI carries its media type and file extension; the serializer is the one
//! that media type names. Nothing here maps format IRIs in code.
use {
    super::names::{FileExtension, MediaType},
    crate::publication::view::RevisionView,
    anyhow::{Result, bail, ensure},
    oxrdf::{Literal, NamedNode, Term},
    oxrdfio::RdfFormat,
};

/// A serialization declared by an RDF format IRI.
#[derive(Clone, Debug)]
pub(super) struct Serialization {
    pub iri:        NamedNode,
    pub media_type: MediaType,
    pub extension:  FileExtension,
    pub syntax:     RdfFormat,
}

impl Serialization {
    /// Read the media type and file extension a format IRI declares.
    pub(super) async fn read(
        view: &RevisionView,
        iri: &NamedNode,
    ) -> Result<Self> {
        let media_type =
            MediaType::try_new(&view.text(iri, "mediaType").await?)?;
        let syntax = RdfFormat::from_media_type(media_type.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "RDF format {iri} declares media type {media_type}, which \
                     no RDF serializer implements"
                )
            })?;
        Ok(Self {
            iri: iri.clone(),
            extension: FileExtension::try_new(
                &view.text(iri, "fileExtension").await?,
            )?,
            media_type,
            syntax,
        })
    }

    /// The one declared RDF format with this media type, naming a source
    /// published as its exact text.
    pub(super) async fn for_media_type(
        view: &RevisionView,
        media_type: &MediaType,
    ) -> Result<Self> {
        let mut rows = view
            .subjects(
                "mediaType",
                &Term::Literal(Literal::new_simple_literal(
                    media_type.as_str(),
                )),
                2,
            )
            .await?;
        let mut found = None;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(iri) = &row["subject"] else {
                bail!("an RDF format declaring {media_type} must be an IRI")
            };
            if !view.has_type(iri, "RdfFormat").await? {
                continue;
            }
            ensure!(
                found.is_none(),
                "several RDF formats declare media type {media_type}"
            );
            found = Some(iri.clone());
        }
        let iri = found.ok_or_else(|| {
            anyhow::anyhow!("no RDF format declares media type {media_type}")
        })?;
        Self::read(view, &iri).await
    }

    /// Whether the syntax represents named graphs.
    pub(super) fn holds_datasets(&self) -> bool {
        self.syntax.supports_datasets()
    }
}

/// What one declared RDF format publishes.
#[derive(Clone, Debug)]
pub(super) enum Format {
    /// The single source's collected bytes, unchanged.
    ExactSource(NamedNode),
    /// The merged sources in one serialization.
    Serialized(Serialization),
}

impl Format {
    pub(super) async fn read(
        view: &RevisionView,
        iri: &NamedNode,
    ) -> Result<Self> {
        ensure!(
            view.has_type(iri, "RdfFormat").await?,
            "{iri} is not a declared ga:RdfFormat"
        );
        if view.has_type(iri, "ExactSourceFormat").await? {
            ensure!(
                view.optional(iri, "mediaType").await?.is_none() &&
                    view.optional(iri, "fileExtension").await?.is_none(),
                "exact source format {iri} takes its media type and file \
                 extension from the collected source"
            );
            return Ok(Self::ExactSource(iri.clone()));
        }
        Ok(Self::Serialized(
            Serialization::read(view, iri).await?,
        ))
    }

    pub(super) fn iri(&self) -> &NamedNode {
        match self {
            Self::ExactSource(iri) => iri,
            Self::Serialized(serialization) => &serialization.iri,
        }
    }
}
