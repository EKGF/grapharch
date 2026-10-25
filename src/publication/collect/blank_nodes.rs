use {
    crate::publication::dataset::identity,
    oxrdf::{BlankNode, GraphName, NamedNode, NamedOrBlankNode, Quad, Term},
    std::collections::HashMap,
};

/// Deterministic source-revision-scoped skolemization. Parser-generated blank
/// IDs are mapped by first encounter, so identical source bytes produce
/// identical RDF.
pub(super) struct BlankNodes {
    scope: String,
    // bounded: blank nodes in one input capped at MAX_RDF_BYTES, never the
    // corpus.
    names: HashMap<BlankNode, NamedNode>,
}

impl BlankNodes {
    pub fn new(
        project: &str,
        collector: &str,
        source: &str,
        revision: &str,
    ) -> Self {
        Self {
            scope: identity("blank-scope", &[
                project, collector, source, revision,
            ])
            .into_string(),
            names: HashMap::new(),
        }
    }

    fn node(&mut self, blank: BlankNode) -> NamedNode {
        let next = self.names.len().to_string();
        self.names
            .entry(blank)
            .or_insert_with(|| identity("skolem", &[&self.scope, &next]))
            .clone()
    }

    pub fn normalize(&mut self, quad: Quad) -> Quad {
        let subject = match quad.subject {
            NamedOrBlankNode::BlankNode(blank) => self.node(blank).into(),
            subject => subject,
        };
        let object = match quad.object {
            Term::BlankNode(blank) => self.node(blank).into(),
            object => object,
        };
        let graph = match quad.graph_name {
            GraphName::BlankNode(blank) => self.node(blank).into(),
            graph => graph,
        };
        Quad::new(subject, quad.predicate, object, graph)
    }
}
