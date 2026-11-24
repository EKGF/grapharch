//! The pinned inputs a class diagram reads beyond its selected facts.
use {
    super::{Node, read},
    crate::publication::{
        dataset::Dataset,
        document::anonymous_order::AnonymousOrder,
        view::RevisionView,
    },
    anyhow::Result,
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet, HashSet},
};

/// The sources a diagram's notation context and expressions are read from.
pub(super) struct Sources<'a> {
    pub dataset:    &'a Dataset,
    pub assertions: &'a RevisionView,
    pub metadata:   &'a RevisionView,
    pub prefixes:   &'a BTreeMap<String, String>,
    pub documented: &'a HashSet<NamedNode>,
}
impl Sources<'_> {
    pub fn order(&self) -> AnonymousOrder<'_> {
        AnonymousOrder::new(self.dataset, self.assertions)
    }

    /// The diagram node of one named resource.
    pub async fn node(&self, id: &NamedNode) -> Result<Node> {
        read::node(
            self.metadata,
            &self.order(),
            id,
            self.prefixes,
            self.documented,
        )
        .await
    }

    /// Named objects of one predicate, at most `limit`, in IRI order.
    pub async fn objects(
        &self,
        subject: &str,
        predicate: &str,
        limit: usize,
    ) -> Result<Option<BTreeSet<NamedNode>>> {
        let mut rows = self
            .metadata
            .objects_by(
                &NamedNode::new(subject)?,
                &NamedNode::new_unchecked(predicate),
            )
            .await?;
        let mut values = BTreeSet::new(); // bounded: `limit` values.
        while let Some(row) = rows.next().await? {
            if let Term::NamedNode(value) = &row["value"] {
                if values.len() == limit {
                    return Ok(None);
                }
                values.insert(value.clone());
            }
        }
        Ok(Some(values))
    }
}
