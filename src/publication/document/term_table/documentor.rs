//! Generates the metadata and details table fragments of one ontology.
use {
    super::{layout::TermTable, names::TermNames},
    crate::publication::{
        RDF_TYPE,
        dataset::{Dataset, Stage, identity},
        diagnostic_graph,
        document::Documentor,
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
};

/// The tables of one ontology of a family reference.
pub(crate) struct TermTableDocumentor {
    pub(super) dataset:      Dataset,
    pub(super) source:       NamedNode,
    pub(super) collected:    NamedNode,
    pub(super) project:      NamedNode,
    pub(super) ontology:     NamedNode,
    pub(super) title:        String,
    pub(super) introduction: String,
    pub(super) names:        TermNames,
    pub(super) table:        TermTable,
    // bounded: the family's FAMILY_RESOURCES term budget.
    pub(super) terms:        Vec<NamedNode>,
    // bounded: the family's FAMILY_RESOURCES term budget.
    pub(super) foreign:      Vec<NamedNode>,
}

/// The two fragments of one ontology's tables.
pub(crate) struct TableFragments {
    pub metadata: NamedNode,
    pub details:  Option<NamedNode>,
}

/// Where the documented ontology's statements are read.
pub(crate) struct OntologySource {
    pub ontology:     NamedNode,
    pub namespace:    String,
    pub title:        String,
    pub introduction: String,
    /// The pinned snapshot of the ontology's own source document.
    pub snapshot:     NamedNode,
    pub terms:        Vec<NamedNode>,
    /// Subjects outside the ontology namespace that its document describes.
    pub foreign:      Vec<NamedNode>,
}

impl TermTableDocumentor {
    pub(crate) fn new(
        dataset: Dataset,
        collected: NamedNode,
        project: &NamedNode,
        source: OntologySource,
        names: (
            std::sync::Arc<std::collections::BTreeMap<String, String>>,
            std::sync::Arc<std::collections::HashSet<NamedNode>>,
        ),
        table: TermTable,
    ) -> Self {
        Self {
            dataset,
            source: source.snapshot,
            collected,
            project: project.clone(),
            names: TermNames::new(names.0, source.namespace, names.1),
            ontology: source.ontology,
            title: source.title,
            introduction: source.introduction,
            table,
            terms: source.terms,
            foreign: source.foreign,
        }
    }

    pub(crate) fn processor_identity(project: &str) -> NamedNode {
        identity("term-table-documentor", &[project])
    }

    /// The contribution source, unique to the ontology and term table.
    pub(crate) fn source_key(&self) -> String {
        format!(
            "{} {}",
            self.ontology.as_str(),
            self.table.identity()
        )
    }

    fn fragment(&self, part: &str) -> NamedNode {
        identity("term-table-fragment", &[
            self.project.as_str(),
            self.ontology.as_str(),
            self.table.identity(),
            part,
        ])
    }

    /// The fragments this documentor writes; an ontology without terms has
    /// no details table.
    pub(crate) fn fragments(&self) -> TableFragments {
        TableFragments {
            metadata: self.fragment("metadata"),
            details:  (!self.terms.is_empty() || !self.foreign.is_empty())
                .then(|| self.fragment("details")),
        }
    }
}

#[async_trait]
impl Documentor for TermTableDocumentor {
    async fn document(&self, writer: &GraphWriter) -> Result<()> {
        let source =
            RevisionView::new(self.dataset.clone(), self.source.clone());
        let collected =
            RevisionView::new(self.dataset.clone(), self.collected.clone());
        let metadata = self.metadata(&source).await?;
        let rows = self.rows(&source, &collected).await?;
        let fragments = self.fragments();
        let details = match &fragments.details {
            Some(_) => Some(self.details(&rows)?),
            None => None,
        };
        let mut hash = Sha256::new();
        hash.update(metadata.as_bytes());
        hash.update([0]);
        hash.update(details.as_deref().unwrap_or_default().as_bytes());
        let revision =
            format!("term-table-v1:{}", hex::encode(hash.finalize()));
        let processor = Self::processor_identity(self.project.as_str());
        let key = self.source_key();
        let contribution = identity("contribution", &[
            self.project.as_str(),
            processor.as_str(),
            &key,
        ]);
        let mut stream = writer
            .begin_stage(
                self.project.as_str(),
                processor.as_str(),
                &key,
                &revision,
                Stage::Document,
            )
            .await?;
        let q = |subject: &NamedNode, predicate: NamedNode, object: Term| {
            Quad::new(
                subject.clone(),
                predicate,
                object,
                GraphName::DefaultGraph,
            )
        };
        let rdf_type = NamedNode::new_unchecked(RDF_TYPE);
        let mut quads = vec![
            q(
                &processor,
                rdf_type.clone(),
                term("DocumentorUse").into(),
            ),
            q(
                &contribution,
                rdf_type.clone(),
                term("Contribution").into(),
            ),
            q(
                &contribution,
                term("project"),
                self.project.clone().into(),
            ),
            q(
                &contribution,
                term("processor"),
                processor.clone().into(),
            ),
            q(
                &contribution,
                term("source"),
                Literal::new_simple_literal(&key).into(),
            ),
            q(
                &contribution,
                term("revision"),
                Literal::new_simple_literal(&revision).into(),
            ),
        ];
        for (fragment, body) in [
            (Some(&fragments.metadata), Some(&metadata)),
            (fragments.details.as_ref(), details.as_ref()),
        ] {
            let (Some(fragment), Some(body)) = (fragment, body) else {
                continue
            };
            quads.extend([
                q(
                    fragment,
                    rdf_type.clone(),
                    term("Fragment").into(),
                ),
                q(
                    fragment,
                    term("subject"),
                    self.ontology.clone().into(),
                ),
                q(
                    fragment,
                    term("body"),
                    Literal::new_simple_literal(body).into(),
                ),
                q(
                    fragment,
                    term("format"),
                    Literal::new_simple_literal("text/markdown").into(),
                ),
                q(
                    fragment,
                    term("contribution"),
                    contribution.clone().into(),
                ),
            ]);
        }
        if let Some(details) = &fragments.details {
            for subject in &self.terms {
                quads.push(q(
                    details,
                    term("rowSubject"),
                    subject.clone().into(),
                ));
            }
        }
        ensure!(
            quads.len() <= 4 * crate::publication::limits::FAMILY_RESOURCES,
            "term table exceeds its statement budget"
        );
        for quad in quads {
            stream.write(quad).await?;
        }
        let context = diagnostic_graph::Context {
            graph:     NamedNode::new_unchecked("urn:grapharch:quality"),
            id:        contribution.clone(),
            project:   self.project.clone(),
            about:     self.ontology.clone(),
            revision:  self.source.clone(),
            processor: processor.clone(),
            source:    key.clone(),
        };
        self.diagnostics(&mut stream, &rows, &context, &revision)
            .await?;
        stream.complete().await?;
        Ok(())
    }
}
