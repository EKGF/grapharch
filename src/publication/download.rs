//! RDF downloads: files a target publishes beside its entry document, built
//! from the exact text of collected RDF sources. Definitions name every file
//! and its format by IRI; renderers never reread the original sources.
use {
    super::{NAMESPACE, RDF_TYPE, dataset::identity, term, view::RevisionView},
    anyhow::{Context, Result, bail, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    oxrdfio::{JsonLdProfileSet, RdfFormat, RdfParser, RdfSerializer},
    std::collections::{BTreeMap, BTreeSet, HashSet},
};

/// bounded: at most this many downloads per target and sources per download.
const MAX_DOWNLOADS: usize = 64;
const MAX_SOURCES: usize = 32;
/// bounded: the combined parsed graph of one download.
const MAX_QUADS: usize = 2_000_000;

/// Identity of the retained text of one RDF collector use.
pub(crate) fn source_text_id(processor: &NamedNode) -> NamedNode {
    identity("rdf-source-text", &[processor.as_str()])
}

/// Statements that retain a collected RDF source's exact text, written by the
/// collector use itself.
pub(crate) fn source_text(
    processor: &NamedNode,
    text: &str,
    media_type: &str,
    base: &str,
) -> Vec<Quad> {
    let subject = source_text_id(processor);
    let quad = |predicate: &str, object: Term| {
        Quad::new(
            subject.clone(),
            term(predicate),
            object,
            GraphName::DefaultGraph,
        )
    };
    vec![
        Quad::new(
            subject.clone(),
            NamedNode::new_unchecked(RDF_TYPE),
            term("RdfSourceText"),
            GraphName::DefaultGraph,
        ),
        quad("body", Literal::new_simple_literal(text).into()),
        quad(
            "format",
            Literal::new_simple_literal(media_type).into(),
        ),
        quad(
            "baseIri",
            Literal::new_simple_literal(base).into(),
        ),
    ]
}

/// How a download represents its sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    /// The single source's exact bytes.
    Source,
    Turtle,
    JsonLd,
    RdfXml,
}

impl Format {
    fn from_iri(iri: &NamedNode) -> Result<Self> {
        Ok(match iri.as_str().strip_prefix(NAMESPACE) {
            Some("SourceFileFormat") => Self::Source,
            Some("TurtleFormat") => Self::Turtle,
            Some("JsonLdFormat") => Self::JsonLd,
            Some("RdfXmlFormat") => Self::RdfXml,
            _ => bail!("unsupported download format {iri}"),
        })
    }

    fn serializer(self) -> Option<RdfFormat> {
        match self {
            Self::Source => None,
            Self::Turtle => Some(RdfFormat::Turtle),
            Self::JsonLd => {
                Some(RdfFormat::JsonLd { profile: JsonLdProfileSet::empty() })
            },
            Self::RdfXml => Some(RdfFormat::RdfXml),
        }
    }
}

struct Source {
    text:   String,
    format: RdfFormat,
    base:   String,
}

impl Source {
    async fn read(view: &RevisionView, processor: &NamedNode) -> Result<Self> {
        ensure!(
            view.has_type(processor, "CollectorUse").await? &&
                view.node(processor, "collectorType").await? ==
                    term("RdfFileCollector"),
            "download source {processor} must be a ga:RdfFileCollector use"
        );
        let id = source_text_id(processor);
        ensure!(
            view.has_type(&id, "RdfSourceText").await?,
            "download source {processor} retains no source text"
        );
        let media = view.text(&id, "format").await?;
        Ok(Self {
            text:   view.text(&id, "body").await?,
            format: RdfFormat::from_media_type(&media).ok_or_else(|| {
                anyhow::anyhow!("unsupported retained RDF media type {media}")
            })?,
            base:   view.text(&id, "baseIri").await?,
        })
    }

    fn parse(
        &self,
        quads: &mut HashSet<Quad>,
        prefixes: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        let mut parser = RdfParser::from_format(self.format)
            .with_base_iri(&self.base)?
            .for_slice(self.text.as_bytes());
        for quad in &mut parser {
            quads.insert(quad?);
            ensure!(
                quads.len() <= MAX_QUADS,
                "download exceeds its {MAX_QUADS}-statement budget"
            );
        }
        for (prefix, iri) in parser.prefixes() {
            prefixes
                .entry(prefix.to_owned())
                .or_insert_with(|| iri.to_owned());
        }
        Ok(())
    }
}

/// One declared download of a target.
pub(crate) struct Download {
    pub file_name: String,
    format:        Format,
    sources:       Vec<NamedNode>,
}

/// A download file name: one lowercase-safe path segment.
fn file_name(download: &NamedNode, value: &str) -> Result<String> {
    ensure!(
        !value.is_empty() &&
            value.len() <= 128 &&
            !value.starts_with('.') &&
            value.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')
            }),
        "download {download} ga:fileName must be one file name of letters, \
         digits, '.', '_' and '-'"
    );
    Ok(value.to_owned())
}

impl Download {
    /// Read and validate every download a target declares.
    pub(crate) async fn read_all(
        view: &RevisionView,
        target: &NamedNode,
    ) -> Result<Vec<Self>> {
        let mut downloads = Vec::new();
        let mut names = BTreeSet::new();
        let mut rows = view.objects(target, "download").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                downloads.len() < MAX_DOWNLOADS,
                "target {target} exceeds {MAX_DOWNLOADS} downloads"
            );
            let Term::NamedNode(download) = &row["value"] else {
                bail!("ga:download must identify a ga:Download")
            };
            ensure!(
                view.has_type(download, "Download").await?,
                "{download} must be a declared ga:Download"
            );
            let name =
                file_name(download, &view.text(download, "fileName").await?)?;
            ensure!(
                names.insert(name.clone()),
                "target {target} declares download {name} more than once"
            );
            let format = Format::from_iri(
                &view.node(download, "downloadFormat").await?,
            )?;
            let mut sources = Vec::new();
            let mut values = view.objects(download, "downloadSource").await?;
            while let Some(row) = values.next().await? {
                ensure!(
                    sources.len() < MAX_SOURCES,
                    "download {download} exceeds {MAX_SOURCES} sources"
                );
                let Term::NamedNode(source) = &row["value"] else {
                    bail!("ga:downloadSource must identify a collector use")
                };
                sources.push(source.clone());
            }
            sources.sort();
            ensure!(
                !sources.is_empty(),
                "download {download} has no source"
            );
            ensure!(
                format != Format::Source || sources.len() == 1,
                "download {download} publishes source bytes and needs exactly \
                 one ga:downloadSource"
            );
            downloads.push(Self { file_name: name, format, sources });
        }
        downloads.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(downloads)
    }

    /// The file's bytes: the exact source text, or the merged source graphs
    /// serialized in the declared format.
    pub(crate) async fn bytes(&self, view: &RevisionView) -> Result<Vec<u8>> {
        let mut sources = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            sources.push(Source::read(view, source).await?);
        }
        let Some(format) = self.format.serializer() else {
            return Ok(sources.remove(0).text.into_bytes());
        };
        let name = self.file_name.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let mut quads = HashSet::new();
            let mut prefixes = BTreeMap::new();
            for source in &sources {
                source.parse(&mut quads, &mut prefixes)?;
            }
            ensure!(
                format != RdfFormat::RdfXml ||
                    quads
                        .iter()
                        .all(|quad| quad.graph_name.is_default_graph()),
                "download {name}: RDF/XML cannot represent named graphs"
            );
            let mut serializer = RdfSerializer::from_format(format);
            for (prefix, iri) in &prefixes {
                serializer = serializer
                    .with_prefix(prefix.clone(), iri.clone())
                    .with_context(|| format!("invalid prefix {prefix}"))?;
            }
            // Deterministic output: statements in lexical order.
            let mut quads = quads.into_iter().collect::<Vec<_>>();
            quads.sort_by_cached_key(ToString::to_string);
            let mut writer = serializer.for_writer(Vec::new());
            for quad in &quads {
                writer.serialize_quad(quad)?;
            }
            Ok(writer.finish()?)
        })
        .await?
    }
}
