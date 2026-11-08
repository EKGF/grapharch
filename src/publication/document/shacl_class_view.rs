//! Read pinned SHACL constraints for class-diagram enrichment.
pub(crate) mod read;
use oxrdf::{NamedNode, Term};
const SH: &str = "http://www.w3.org/ns/shacl#";

pub(super) struct Property {
    pub(super) id:    NamedNode,
    pub(super) paths: Vec<Term>, /* bounded: at most sixty-four facts per
                                  * property shape. */
    pub(super) facts: Vec<(NamedNode, Term)>,
}
pub(super) struct Shape {
    pub(super) id:         NamedNode,
    pub(super) properties: Vec<Property>, /* bounded: thirty-two property
                                           * shapes. */
}
