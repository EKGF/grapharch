//! The exact text of a collected RDF source, retained in the knowledge graph
//! by the collector use that an RDF renderer target names as a source.
use {
    super::names::MediaType,
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        term,
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    oxrdfio::{RdfFormat, RdfParser},
    std::collections::{BTreeMap, HashSet},
};

/// bounded: the merged parsed graph of one RDF target.
pub(super) const MAX_QUADS: usize = 2_000_000;

/// Retained text of one RDF collector use.
pub(crate) struct SourceText {
    pub text:       String,
    pub media_type: MediaType,
    pub base:       String,
}

impl SourceText {
    fn id(processor: &NamedNode) -> NamedNode {
        identity("rdf-source-text", &[processor.as_str()])
    }

    pub(crate) fn new(
        text: String,
        media_type: &str,
        base: &str,
    ) -> Result<Self> {
        Ok(Self {
            text,
            media_type: MediaType::try_new(media_type)?,
            base: base.to_owned(),
        })
    }

    /// Statements retaining this text, written by its collector use.
    pub(crate) fn quads(&self, processor: &NamedNode) -> Vec<Quad> {
        let subject = Self::id(processor);
        let quad = |predicate: NamedNode, object: Term| {
            Quad::new(
                subject.clone(),
                predicate,
                object,
                GraphName::DefaultGraph,
            )
        };
        // bounded: four fixed statements.
        vec![
            quad(
                NamedNode::new_unchecked(RDF_TYPE),
                term("RdfSourceText").into(),
            ),
            quad(
                term("body"),
                Literal::new_simple_literal(&self.text).into(),
            ),
            quad(
                term("format"),
                Literal::new_simple_literal(self.media_type.as_str()).into(),
            ),
            quad(
                term("baseIri"),
                Literal::new_simple_literal(&self.base).into(),
            ),
        ]
    }

    /// Read the text a collector use retained for an RDF target.
    pub(crate) async fn read(
        view: &RevisionView,
        processor: &NamedNode,
    ) -> Result<Self> {
        ensure!(
            view.has_type(processor, "CollectorUse").await? &&
                view.node(processor, "collectorType").await? ==
                    term("RdfFileCollector"),
            "RDF source {processor} must be a ga:RdfFileCollector use"
        );
        let id = Self::id(processor);
        ensure!(
            view.has_type(&id, "RdfSourceText").await?,
            "RDF source {processor} retains no source text"
        );
        Self::new(
            view.text(&id, "body").await?,
            &view.text(&id, "format").await?,
            &view.text(&id, "baseIri").await?,
        )
    }

    /// Parse into the merged statements, keeping the first declaration of
    /// each prefix.
    pub(super) fn parse(
        &self,
        quads: &mut HashSet<Quad>,
        prefixes: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        let syntax = RdfFormat::from_media_type(self.media_type.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unsupported retained RDF media type {}",
                    self.media_type
                )
            })?;
        let mut parser = RdfParser::from_format(syntax)
            .with_base_iri(&self.base)?
            .for_slice(self.text.as_bytes());
        for quad in &mut parser {
            quads.insert(quad?);
            ensure!(
                quads.len() <= MAX_QUADS,
                "RDF target exceeds its {MAX_QUADS}-statement budget"
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
