//! SPARQL update patterns for single quads.
use oxrdf::{GraphNameRef, QuadRef};

pub(in crate::publication::dataset::backend) fn quad_pattern(
    quad: QuadRef<'_>,
) -> String {
    let triple = format!(
        "{} {} {} .",
        quad.subject, quad.predicate, quad.object
    );
    match quad.graph_name {
        GraphNameRef::DefaultGraph => triple,
        graph => format!("GRAPH {graph} {{ {triple} }}"),
    }
}
