//! Each classified reference as RDF, so links are queryable: backlinks, the
//! diagram index and the broken-link report.
use {
    super::{form::ReferenceForm, kind::Classification},
    crate::publication::{RDF_TYPE, dataset::identity, term},
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad},
    std::sync::Mutex,
};

/// Published file holding the reference records.
pub(crate) const FILE: &str = "references.nq";

/// The named graph of one target's reference records.
pub(crate) fn graph(target: &NamedNode) -> NamedNode {
    identity("references", &[target.as_str()])
}

/// One decided reference.
struct Recorded {
    source:   String,
    original: String,
    form:     &'static str,
    kind:     &'static str,
    target:   Option<String>,
    reason:   Option<String>,
}

/// The references decided while rendering one publication.
#[derive(Default)]
pub(crate) struct ReferenceRecorder {
    // bounded: at most LINK_INDEX_ENTRIES references.
    entries: Mutex<Vec<Recorded>>,
}

impl ReferenceRecorder {
    pub(crate) fn record(
        &self,
        source: &str,
        original: &str,
        classification: &Classification,
    ) -> Result<()> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("reference recorder"))?;
        ensure!(
            entries.len() < crate::publication::limits::LINK_INDEX_ENTRIES,
            "reference records exceed {}",
            crate::publication::limits::LINK_INDEX_ENTRIES
        );
        entries.push(Recorded {
            source:   source.to_owned(),
            original: original.to_owned(),
            form:     ReferenceForm::parse(original).term(),
            kind:     classification.kind.term(),
            target:   classification.href.clone(),
            reason:   match &classification.kind {
                super::ReferenceKind::Broken(reason) => Some(reason.clone()),
                _ => None,
            },
        });
        Ok(())
    }

    /// The records as N-Quads in `graph`, one `ga:Reference` per distinct
    /// source and original form.
    pub(crate) fn nquads(&self, graph: &NamedNode) -> Result<String> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("reference recorder"))?;
        let mut out = String::new();
        let mut seen = std::collections::BTreeSet::new(); // bounded: entries.
        for entry in entries.iter() {
            if !seen.insert((&entry.source, &entry.original)) {
                continue;
            }
            let node = identity("reference", &[&entry.source, &entry.original]);
            let mut put = |predicate: NamedNode, object: oxrdf::Term| {
                out.push_str(
                    &Quad::new(
                        node.clone(),
                        predicate,
                        object,
                        GraphName::NamedNode(graph.clone()),
                    )
                    .to_string(),
                );
                out.push_str(" .\n");
            };
            put(
                NamedNode::new_unchecked(RDF_TYPE),
                term("Reference").into(),
            );
            put(
                term("referenceSource"),
                Literal::new_simple_literal(&entry.source).into(),
            );
            put(
                term("originalReference"),
                Literal::new_simple_literal(&entry.original).into(),
            );
            put(term("referenceForm"), term(entry.form).into());
            put(term("referenceKind"), term(entry.kind).into());
            if let Some(target) = &entry.target {
                put(
                    term("referenceTarget"),
                    Literal::new_simple_literal(target).into(),
                );
            }
            if let Some(reason) = &entry.reason {
                put(
                    term("message"),
                    Literal::new_simple_literal(reason).into(),
                );
            }
        }
        Ok(out)
    }
}
