//! The anonymous class axioms of one class, read within the diagram budget.
use {
    super::{
        model::{AxiomRelation, ClassAxiom},
        read::{ExpressionReader, Unreadable},
    },
    crate::publication::document::class_diagram::{
        Edge,
        Node,
        read::compact,
        sources::Sources,
    },
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
};
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const SKOLEM: &str = "urn:grapharch:skolem:";

/// The readable axioms of one class, the authored facts of malformed ones,
/// and whether any axiom exceeded the budget.
#[derive(Default)]
pub(in super::super) struct Axioms {
    pub axioms:   Vec<ClassAxiom>,
    /// Malformed expressions keep their authored facts, one group each.
    pub fallback: Vec<Vec<Edge>>, // bounded: `limit` groups.
    pub limited:  bool,
}
impl AxiomRelation {
    pub(in super::super) const ALL: [Self; 4] = [
        Self::Subclass,
        Self::Equivalent,
        Self::Disjoint,
        Self::Superclass,
    ];

    fn slug(self) -> &'static str {
        match self {
            Self::Subclass => "subclass",
            Self::Equivalent => "equivalent",
            Self::Disjoint => "disjoint",
            Self::Superclass => "superclass",
        }
    }

    fn predicate(self) -> String {
        match self {
            Self::Subclass | Self::Superclass => SUBCLASS.to_owned(),
            Self::Equivalent => format!("{OWL}equivalentClass"),
            Self::Disjoint => format!("{OWL}disjointWith"),
        }
    }

    /// The SPARQL pattern binding `?x` to an expression in this relation.
    fn pattern(self, class: &NamedNode) -> String {
        let predicate = self.predicate();
        let forward = format!("{{ {class} <{predicate}> ?x }}");
        let backward = format!("{{ ?x <{predicate}> {class} }}");
        let pattern = match self {
            Self::Subclass => forward,
            Self::Superclass => backward,
            Self::Equivalent | Self::Disjoint => {
                format!("{forward} UNION {backward}")
            },
        };
        format!("{{ {pattern} BIND(\"{}\" AS ?r) }}", self.slug())
    }

    /// The authored relation fact between the class and the expression.
    fn edge(self, class: &Node, expression: &Node, sources: &Sources) -> Edge {
        let predicate = self.predicate();
        let (from, to) = if self == Self::Superclass {
            (expression, class)
        } else {
            (class, expression)
        };
        Edge {
            from:      from.clone(),
            relation:  compact(&predicate, sources.prefixes),
            predicate: Some(predicate),
            to:        to.clone(),
        }
    }
}
impl Axioms {
    /// At most `limit` axioms of `relations` on `class`, in content order.
    pub async fn read(
        sources: &Sources<'_>,
        class: &Node,
        relations: &[AxiomRelation],
        limit: usize,
    ) -> Result<Self> {
        let id = NamedNode::new(class.id.as_str())?;
        let patterns: Vec<String> = AxiomRelation::ALL
            .into_iter()
            .filter(|r| relations.contains(r))
            .map(|r| r.pattern(&id))
            .collect(); // bounded: four relations.
        let pattern = sources.assertions.pattern(&patterns.join(" UNION "), 48);
        let mut rows = sources
            .dataset
            .select(format!(
                "SELECT DISTINCT ?x ?r WHERE {{ {pattern} \
                 FILTER(STRSTARTS(STR(?x), \"{SKOLEM}\")) }} LIMIT {}",
                limit + 1
            ))
            .await?;
        let order = sources.order();
        let mut found = Vec::new(); // bounded: `limit` plus one sentinel.
        while let Some(row) = rows.next().await? {
            let (Term::NamedNode(node), Term::Literal(slug)) =
                (&row["x"], &row["r"])
            else {
                bail!("class axiom must name an anonymous expression");
            };
            let Some(relation) = AxiomRelation::ALL
                .into_iter()
                .find(|r| r.slug() == slug.value())
            else {
                bail!("unknown class axiom relation");
            };
            found.push((relation, order.digest(node).await?, node.clone()));
        }
        drop(rows);
        found.sort();
        let mut axioms =
            Self { limited: found.len() > limit, ..Self::default() };
        let mut reader = ExpressionReader::new(sources);
        for (relation, digest, node) in found.into_iter().take(limit) {
            let id = format!("expression-{digest}");
            match reader.read(&node).await? {
                Ok(expression) => {
                    axioms.axioms.push(ClassAxiom {
                        class: class.clone(),
                        relation,
                        id,
                        expression,
                    })
                },
                Err(Unreadable::OverBudget) => axioms.limited = true,
                Err(Unreadable::Malformed) => {
                    axioms.fallback.push(
                        Self::authored(sources, class, relation, &node).await?,
                    );
                },
            }
        }
        Ok(axioms)
    }

    /// The relation fact and the direct OWL facts of a malformed expression.
    async fn authored(
        sources: &Sources<'_>,
        class: &Node,
        relation: AxiomRelation,
        node: &NamedNode,
    ) -> Result<Vec<Edge>> {
        let expression = sources.node(node).await?;
        let mut edges = vec![relation.edge(class, &expression, sources)]; // bounded: sixty-four facts.
        for (predicate, value) in sources.order().facts(node).await? {
            if !predicate.as_str().starts_with(OWL) {
                continue;
            }
            let to = match &value {
                Term::NamedNode(value) => sources.node(value).await?,
                Term::Literal(value) => Node::literal(value),
                _ => continue,
            };
            edges.push(Edge {
                from: expression.clone(),
                relation: compact(predicate.as_str(), sources.prefixes),
                predicate: Some(predicate.as_str().into()),
                to,
            });
        }
        Ok(edges)
    }
}
