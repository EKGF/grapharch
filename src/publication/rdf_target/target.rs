//! One RDF renderer target: its collected sources, its declared formats and
//! the file each format publishes.
use {
    super::{
        format::{Format, Serialization},
        merged::MergedSources,
        names::{MediaType, RdfFileName},
        negotiation::Negotiation,
        source_text::SourceText,
    },
    crate::publication::{pdf_name::Slug, view::RevisionView},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    serde::{Deserialize, Serialize},
    std::{collections::BTreeSet, path::Path},
};

/// bounded: sources and formats one RDF target may declare.
const MAX_SOURCES: usize = 32;
const MAX_FORMATS: usize = 8;

/// A file an RDF target published, with the media type it is served as.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublishedRdfFile {
    pub name:       RdfFileName,
    pub media_type: MediaType,
}

pub(crate) struct RdfTarget {
    pub id:          NamedNode,
    pub slug:        Slug,
    sources:         Vec<NamedNode>,
    formats:         Vec<Format>,
    pub negotiation: Option<Negotiation>,
}

impl RdfTarget {
    /// Read and validate an RDF target definition.
    pub(crate) async fn read(
        view: &RevisionView,
        project: &NamedNode,
        target: &NamedNode,
    ) -> Result<Self> {
        let slug = match view.optional(target, "slug").await? {
            Some(Term::Literal(value)) => Slug::try_new(value.value())?,
            _ => {
                bail!("RDF target {target} requires a ga:slug naming its files")
            },
        };
        let sources =
            Self::iris(view, target, "rdfSource", MAX_SOURCES).await?;
        ensure!(
            !sources.is_empty(),
            "RDF target {target} has no ga:rdfSource"
        );
        let mut formats = Vec::new();
        for iri in Self::iris(view, target, "rdfFormat", MAX_FORMATS).await? {
            formats.push(Format::read(view, &iri).await?);
        }
        ensure!(
            !formats.is_empty(),
            "RDF target {target} has no ga:rdfFormat"
        );
        if let Some(exact) =
            formats.iter().find(|f| matches!(f, Format::ExactSource(_)))
        {
            ensure!(
                sources.len() == 1,
                "RDF target {target} publishes the exact source text ({}) and \
                 needs exactly one ga:rdfSource",
                exact.iri()
            );
        }
        Ok(Self {
            id: target.clone(),
            slug,
            sources,
            formats,
            negotiation: Negotiation::read(view, project, target).await?,
        })
    }

    async fn iris(
        view: &RevisionView,
        target: &NamedNode,
        property: &str,
        limit: usize,
    ) -> Result<Vec<NamedNode>> {
        let mut values = Vec::new();
        let mut rows = view.objects(target, property).await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                values.len() < limit,
                "RDF target {target} exceeds {limit} ga:{property} values"
            );
            let Term::NamedNode(value) = &row["value"] else {
                bail!("RDF target {target} ga:{property} must be an IRI")
            };
            values.push(value.clone());
        }
        values.sort();
        Ok(values)
    }

    /// Write one file per declared format into `stage`, returning them in
    /// file-name order.
    pub(crate) async fn write(
        &self,
        view: &RevisionView,
        stage: &Path,
    ) -> Result<Vec<PublishedRdfFile>> {
        let mut sources = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            sources.push(SourceText::read(view, source).await?);
        }
        let mut serializations = Vec::new();
        let mut planned = Vec::new();
        for format in &self.formats {
            match format {
                Format::ExactSource(_) => {
                    let declared = Serialization::for_media_type(
                        view,
                        &sources[0].media_type,
                    )
                    .await?;
                    planned.push((format.iri().clone(), declared, true));
                },
                Format::Serialized(serialization) => {
                    serializations.push(serialization.clone());
                    planned.push((
                        format.iri().clone(),
                        serialization.clone(),
                        false,
                    ));
                },
            }
        }
        let mut names = BTreeSet::new();
        for (iri, serialization, _) in &planned {
            let name = RdfFileName::new(&self.slug, &serialization.extension);
            ensure!(
                names.insert(name.clone()),
                "RDF target {} publishes {name} for more than one format, \
                 including {iri}",
                self.id
            );
        }
        let stage = stage.to_owned();
        let slug = self.slug.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<PublishedRdfFile>> {
            let merged = if serializations.is_empty() {
                None
            } else {
                Some(MergedSources::parse(&sources)?)
            };
            let mut files = Vec::with_capacity(planned.len());
            for (_, serialization, verbatim) in &planned {
                let name = RdfFileName::new(&slug, &serialization.extension);
                let bytes = if *verbatim {
                    sources[0].text.as_bytes().to_vec()
                } else {
                    merged
                        .as_ref()
                        .ok_or_else(|| {
                            anyhow::anyhow!("merged sources were not parsed")
                        })?
                        .serialize(serialization)?
                };
                std::fs::write(stage.join(name.as_str()), bytes)?;
                files.push(PublishedRdfFile {
                    name,
                    media_type: serialization.media_type.clone(),
                });
            }
            files.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(files)
        })
        .await?
    }
}
