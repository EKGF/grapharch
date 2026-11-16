//! Reads anonymous descriptions and arranges facts by content.
use {
    super::key::{ContentKey, Rank, ShaclOrder, TermKey},
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    futures::future::BoxFuture,
    oxrdf::{NamedNode, Term},
    sha2::{Digest, Sha256},
};

const SKOLEM: &str = "urn:grapharch:skolem:";
const SH: &str = "http://www.w3.org/ns/shacl#";
const REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";

/// One retained fact of a described resource.
pub(in crate::publication::document) type Fact = (NamedNode, Term);

/// Reads anonymous descriptions from one pinned revision.
pub(in crate::publication::document) struct AnonymousOrder<'a> {
    dataset: &'a Dataset,
    view:    &'a RevisionView,
}

impl<'a> AnonymousOrder<'a> {
    pub fn new(dataset: &'a Dataset, view: &'a RevisionView) -> Self {
        Self { dataset, view }
    }

    /// Whether `term` is a skolemized blank node.
    pub fn is_anonymous(term: &Term) -> bool {
        matches!(term, Term::NamedNode(node) if node.as_str().starts_with(SKOLEM))
    }

    /// The direct facts of an anonymous resource, at most sixty-four.
    pub async fn facts(&self, node: &NamedNode) -> Result<Vec<Fact>> {
        let pattern =
            self.view.pattern(&format!("{node} ?predicate ?object"), 3);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?predicate ?object WHERE {{ {pattern} }} \
                 ORDER BY ?predicate ?object LIMIT 65"
            ))
            .await?;
        let mut facts = Vec::new(); // bounded: sixty-four facts plus sentinel.
        while let Some(row) = rows.next().await? {
            ensure!(
                facts.len() < 64,
                "anonymous RDF expression exceeds fact budget for {node}"
            );
            let Term::NamedNode(predicate) = &row["predicate"] else {
                anyhow::bail!("RDF predicate must be named")
            };
            facts.push((predicate.clone(), row["object"].clone()));
        }
        Ok(facts)
    }

    /// Stable-sort `facts` by predicate, then by content-derived value rank.
    /// The caller bounds `facts`.
    pub async fn arrange(&self, facts: Vec<Fact>) -> Result<Vec<Fact>> {
        let mut ranked = Vec::new(); // bounded: the caller's fact budget.
        for (index, (predicate, value)) in facts.into_iter().enumerate() {
            let rank = match &value {
                Term::NamedNode(node) if Self::is_anonymous(&value) => {
                    Rank::Anonymous(self.key(node).await?)
                },
                _ => Rank::Plain(index),
            };
            ranked.push((predicate, rank, value));
        }
        ranked.sort_by(|a, b| {
            a.0.as_str().cmp(b.0.as_str()).then_with(|| a.1.cmp(&b.1))
        });
        Ok(ranked.into_iter().map(|(p, _, v)| (p, v)).collect())
    }

    /// The distinct `?group` resources selected by `selection`, at most 256,
    /// in content order.
    pub async fn groups(&self, selection: &str) -> Result<Vec<NamedNode>> {
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?group WHERE {{ {selection} }} LIMIT 257"
            ))
            .await?;
        let mut groups = Vec::new(); // bounded: 256 groups plus sentinel.
        while let Some(row) = rows.next().await? {
            ensure!(groups.len() < 256, "selection exceeds 256 groups");
            let Term::NamedNode(group) = &row["group"] else {
                anyhow::bail!("selected group must be a resource")
            };
            let key = self.term_key(&row["group"]).await?;
            groups.push((key, group.clone()));
        }
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(groups.into_iter().map(|(_, group)| group).collect())
    }

    /// Sort `[subject, predicate, object]` rows by the content keys of their
    /// terms. The caller bounds `rows`.
    pub async fn sort_statements(
        &self,
        rows: Vec<[Term; 3]>,
    ) -> Result<Vec<[Term; 3]>> {
        let mut ranked = Vec::new(); // bounded: the caller's row budget.
        for row in rows {
            let key = [
                self.term_key(&row[0]).await?,
                self.term_key(&row[1]).await?,
                self.term_key(&row[2]).await?,
            ];
            ranked.push((key, row));
        }
        ranked.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(ranked.into_iter().map(|(_, row)| row).collect())
    }

    /// Presentation key of any value.
    async fn term_key(&self, term: &Term) -> Result<TermKey> {
        Ok(match term {
            Term::NamedNode(node) if Self::is_anonymous(term) => {
                TermKey::Anonymous(self.key(node).await?)
            },
            term => TermKey::Plain(term.to_string()),
        })
    }

    /// Hex digest of the canonical description of an anonymous resource,
    /// a stable identity independent of its skolem IRI.
    pub async fn digest(&self, node: &NamedNode) -> Result<String> {
        let canonical = self
            .canonical(node.clone(), &mut Vec::new(), 0, &mut 0)
            .await?;
        Ok(hex::encode(Sha256::digest(canonical.as_bytes())))
    }

    async fn key(&self, node: &NamedNode) -> Result<ContentKey> {
        let facts = self.facts(node).await?;
        let mut order = None;
        let mut labels = Vec::new(); // bounded: sixty-four facts.
        for (predicate, value) in &facts {
            match predicate.as_str().strip_prefix(SH) {
                Some("order") => {
                    ensure!(order.is_none(), "{node} has several sh:order");
                    order = Some(ShaclOrder::try_new(value)?);
                },
                Some("name" | "message") => {
                    if let Term::Literal(literal) = value {
                        labels.push(literal.value().to_owned());
                    }
                },
                _ => {},
            }
        }
        labels.sort();
        let canonical = self
            .canonical(node.clone(), &mut Vec::new(), 0, &mut 0)
            .await?;
        Ok(ContentKey {
            order,
            label: (!labels.is_empty()).then(|| labels.join("\u{1f}")),
            digest: Sha256::digest(canonical.as_bytes()).into(),
        })
    }

    /// Canonical text of the anonymous tree rooted at `node`: nested anonymous
    /// values are replaced by their own canonical text and facts are sorted,
    /// so it is independent of skolem identity. List cells are followed
    /// iteratively and keep their sequence; a cycle is written as the stack
    /// position it returns to.
    fn canonical<'b>(
        &'b self,
        node: NamedNode,
        path: &'b mut Vec<NamedNode>,
        depth: usize,
        facts_read: &'b mut usize,
    ) -> BoxFuture<'b, Result<String>> {
        Box::pin(async move {
            ensure!(
                depth <= 24,
                "anonymous RDF expression exceeds depth budget"
            );
            let mut cells = Vec::new(); // bounded: 4096 list cells.
            let entry = path.len();
            let mut cell = node;
            loop {
                ensure!(
                    cells.len() < 4096,
                    "RDF collection exceeds budget"
                );
                // A cycle is described by the stack position it returns to,
                // never by the skolem identity.
                if let Some(position) =
                    path.iter().position(|seen| seen == &cell)
                {
                    cells.push(format!("^{position}"));
                    break;
                }
                path.push(cell.clone()); // bounded: depth and list budgets.
                let facts = self.facts(&cell).await?;
                *facts_read += facts.len();
                ensure!(
                    *facts_read <= 16 * 1024,
                    "anonymous RDF expression exceeds ordering budget"
                );
                let mut pairs = Vec::new(); // bounded: sixty-four facts.
                let mut next = None;
                for (predicate, value) in facts {
                    if predicate.as_str() == REST &&
                        let Term::NamedNode(rest) = &value &&
                        rest.as_str().starts_with(SKOLEM)
                    {
                        next = Some(rest.clone());
                        continue;
                    }
                    let value = match value {
                        Term::NamedNode(child)
                            if child.as_str().starts_with(SKOLEM) =>
                        {
                            self.canonical(child, path, depth + 1, facts_read)
                                .await?
                        },
                        value => value.to_string(),
                    };
                    pairs.push(format!("{predicate} {value}"));
                }
                pairs.sort();
                cells.push(format!("[{}]", pairs.join(" ; ")));
                match next {
                    Some(rest) => cell = rest,
                    None => break,
                }
            }
            path.truncate(entry);
            Ok(cells.join(" , "))
        })
    }
}
