//! Separate exact-target SHACL views; never reinterpret validation as OWL
//! axioms.
mod read;
mod render;
use {
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::Result,
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, HashSet},
};
const SH: &str = "http://www.w3.org/ns/shacl#";

struct Property {
    id:    NamedNode,
    paths: Vec<Term>, // bounded: at most sixty-four facts per property shape.
    facts: Vec<(NamedNode, Term)>,
}
struct Shape {
    id:         NamedNode,
    facts:      Vec<(NamedNode, Term)>, // bounded: sixty-four source facts.
    properties: Vec<Property>,          // bounded: thirty-two property shapes.
}

pub(super) async fn append(
    dataset: &Dataset,
    view: &RevisionView,
    class: &NamedNode,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
    body: &mut String,
) -> Result<()> {
    let shapes = read::shapes(dataset, view, class).await?;
    if shapes.is_empty() {
        return Ok(());
    }
    render::append(&shapes, prefixes, documented, body)
}
