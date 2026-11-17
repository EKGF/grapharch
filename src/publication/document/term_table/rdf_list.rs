//! Bounded reading of RDF collections from one pinned view.
use {
    crate::publication::view::RevisionView,
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::HashSet,
};

const FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// The most members a configured or source RDF collection may hold.
pub(super) const MEMBERS: usize = 256;

/// One RDF collection, identified by its head cell.
pub(super) struct RdfList<'a> {
    view: &'a RevisionView,
    head: Term,
}

impl<'a> RdfList<'a> {
    pub fn new(view: &'a RevisionView, head: Term) -> Self {
        Self { view, head }
    }

    /// The members in collection order. A cell needs exactly one `rdf:first`
    /// and one `rdf:rest`; cycles and more than [`MEMBERS`] members fail.
    pub async fn members(&self) -> Result<Vec<Term>> {
        let first = NamedNode::new_unchecked(FIRST);
        let rest = NamedNode::new_unchecked(REST);
        let mut members = Vec::new(); // bounded: MEMBERS.
        let mut seen = HashSet::new(); // bounded: MEMBERS.
        let mut cell = self.head.clone();
        loop {
            let Term::NamedNode(node) = &cell else {
                bail!("an RDF collection cell must be a resource")
            };
            if node.as_str() == NIL {
                return Ok(members);
            }
            ensure!(seen.insert(node.clone()), "cyclic RDF collection");
            ensure!(
                members.len() < MEMBERS,
                "RDF collection exceeds {MEMBERS} members"
            );
            let (Some(value), Some(next)) = (
                self.view.optional_by(node, &first).await?,
                self.view.optional_by(node, &rest).await?,
            ) else {
                bail!(
                    "malformed RDF collection: each cell needs one rdf:first \
                     and one rdf:rest"
                )
            };
            members.push(value);
            cell = next;
        }
    }

    /// The members, each required to be an IRI.
    pub async fn resources(&self, field: &str) -> Result<Vec<NamedNode>> {
        self.members()
            .await?
            .into_iter()
            .map(|member| {
                match member {
                    Term::NamedNode(node) => Ok(node),
                    _ => bail!("every {field} member must be an IRI"),
                }
            })
            .collect()
    }
}
