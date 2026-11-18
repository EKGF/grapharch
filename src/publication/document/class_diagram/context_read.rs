//! Read the bounded notation context of one selected neighbourhood.
use {
    super::{
        Diagram,
        Edge,
        Node,
        NodeKind,
        context::{Context, PROPERTY_USES, PropertyUse, RESTRICTION_FACTS},
        read,
        restriction::Restriction,
        shorthand::{GENERIC_RESOURCE, Shorthand},
    },
    crate::publication::{
        dataset::Dataset,
        document::anonymous_order::AnonymousOrder,
        view::RevisionView,
    },
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet, HashSet},
};
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const DOMAIN: &str = "http://www.w3.org/2000/01/rdf-schema#domain";
const RANGE: &str = "http://www.w3.org/2000/01/rdf-schema#range";
/// Neighbouring classes inspected for their own restrictions.
const NEIGHBOURS: usize = 12;
/// Rows read per neighbour; reaching it skips that neighbour whole.
const NEIGHBOUR_ROWS: usize = 32;
/// Superclasses visited when deciding whether a property is inherited.
const ANCESTORS: usize = 64;

/// The sources a context is read from.
pub(super) struct Sources<'a> {
    pub dataset:    &'a Dataset,
    pub assertions: &'a RevisionView,
    pub metadata:   &'a RevisionView,
    pub prefixes:   &'a BTreeMap<String, String>,
    pub documented: &'a HashSet<NamedNode>,
}
impl Sources<'_> {
    async fn node(&self, id: &NamedNode) -> Result<Node> {
        let order = AnonymousOrder::new(self.dataset, self.assertions);
        read::node(
            self.metadata,
            &order,
            id,
            self.prefixes,
            self.documented,
        )
        .await
    }

    /// Named objects of one predicate, at most `limit`, in IRI order.
    async fn objects(
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
impl Context {
    pub(super) async fn read(
        diagram: &Diagram,
        sources: &Sources<'_>,
    ) -> Result<Self> {
        let mut context = Self {
            restrictions: Self::neighbours(diagram, sources).await?,
            properties:   Vec::new(),
        };
        let mut pairs = BTreeSet::new(); // bounded: PROPERTY_USES pairs.
        for restriction in Restriction::collect(&diagram.edges)
            .into_iter()
            .chain(Restriction::collect(&context.restrictions))
        {
            if pairs.len() == PROPERTY_USES {
                break;
            }
            pairs.insert((
                restriction.class.id.clone(),
                restriction.property.id.clone(),
            ));
        }
        for (class, property) in pairs {
            context
                .properties
                .push(Self::read_use(sources, class, property).await?);
        }
        context.canonicalize(diagram);
        Ok(context)
    }

    async fn read_use(
        sources: &Sources<'_>,
        class: String,
        property: String,
    ) -> Result<PropertyUse> {
        let mut ranges = Vec::new(); // bounded: four declared ranges.
        for range in sources
            .objects(&property, RANGE, 4)
            .await?
            .unwrap_or_default()
        {
            if range.as_str() != GENERIC_RESOURCE {
                ranges.push(sources.node(&range).await?);
            }
        }
        let inherited = match sources.objects(&property, DOMAIN, 16).await? {
            Some(domains) if !domains.is_empty() => {
                !domains.iter().any(|d| d.as_str() == class) &&
                    Self::inherits(sources, &class, &domains).await?
            },
            _ => false,
        };
        Ok(PropertyUse { class, property, ranges, inherited })
    }

    /// Use the focus neighbourhood's node wherever the same resource appears,
    /// so one identity never carries two different presentations.
    fn canonicalize(&mut self, diagram: &Diagram) {
        let known: BTreeMap<&str, &Node> =
            diagram.nodes().map(|n| (n.id.as_str(), n)).collect(); // bounded: the selected diagram nodes.
        for node in self.nodes_mut() {
            if let Some(canonical) = known.get(node.id.as_str()) {
                *node = (*canonical).clone();
            }
        }
    }

    /// Whether an asserted superclass chain reaches a declared domain.
    async fn inherits(
        sources: &Sources<'_>,
        class: &str,
        domains: &BTreeSet<NamedNode>,
    ) -> Result<bool> {
        let mut pending = vec![class.to_owned()]; // bounded: ANCESTORS.
        let mut visited = BTreeSet::new(); // bounded: ANCESTORS.
        while let Some(current) = pending.pop() {
            if !visited.insert(current.clone()) || visited.len() > ANCESTORS {
                continue;
            }
            for parent in sources
                .objects(&current, SUBCLASS, 16)
                .await?
                .unwrap_or_default()
            {
                if domains.contains(&parent) {
                    return Ok(true);
                }
                pending.push(parent.as_str().to_owned());
            }
        }
        Ok(false)
    }

    /// Complete simple restrictions on visible neighbouring classes.
    async fn neighbours(
        diagram: &Diagram,
        sources: &Sources<'_>,
    ) -> Result<Vec<Edge>> {
        let visible = Shorthand::new(diagram).visible;
        let classes: BTreeSet<&str> = diagram
            .nodes()
            .filter(|n| {
                n.kind == NodeKind::Class &&
                    n.id != diagram.focus.id &&
                    visible.contains(n.id.as_str())
            })
            .map(|n| n.id.as_str())
            .take(NEIGHBOURS)
            .collect(); // bounded: twelve neighbours.
        let mut edges = Vec::new(); // bounded: RESTRICTION_FACTS facts.
        for class in classes {
            let node = diagram
                .nodes()
                .find(|n| n.id == class)
                .cloned()
                .unwrap_or_else(|| unreachable!("selected from the diagram"));
            for group in Self::groups(sources, &node).await? {
                if edges.len() + group.len() > RESTRICTION_FACTS {
                    return Ok(edges);
                }
                // A restriction already in the focus neighbourhood is drawn
                // there; never describe one node twice.
                let shared = group.iter().any(|edge| {
                    diagram.nodes().any(|n| {
                        n.kind == NodeKind::OwlExpression && n.id == edge.to.id
                    })
                });
                if !shared && !Restriction::collect(&group).is_empty() {
                    edges.extend(group);
                }
            }
        }
        Ok(edges)
    }

    /// The restriction groups of one class, each with its subclass fact.
    async fn groups(
        sources: &Sources<'_>,
        class: &Node,
    ) -> Result<Vec<Vec<Edge>>> {
        let id = NamedNode::new(class.id.as_str())?;
        let pattern = sources.assertions.pattern(
            &format!(
                "{id} <{SUBCLASS}> ?s . ?s a <{OWL}Restriction> ; ?p ?o . \
                 FILTER(STRSTARTS(STR(?p), \"{OWL}\")) FILTER(isIRI(?s) && \
                 (isIRI(?o) || isLiteral(?o)))"
            ),
            47,
        );
        let mut rows = sources
            .dataset
            .select(format!(
                "SELECT DISTINCT ?s ?p ?o WHERE {{ {pattern} }} ORDER BY ?s \
                 ?p ?o LIMIT {}",
                NEIGHBOUR_ROWS + 1
            ))
            .await?;
        let mut facts = Vec::new(); // bounded: NEIGHBOUR_ROWS + 1 rows.
        while let Some(row) = rows.next().await? {
            facts.push([row["s"].clone(), row["p"].clone(), row["o"].clone()]);
        }
        if facts.len() > NEIGHBOUR_ROWS {
            return Ok(Vec::new());
        }
        let mut groups = BTreeMap::<String, Vec<Edge>>::new(); // bounded: the rows above.
        for [s, p, o] in facts {
            let (Term::NamedNode(s), Term::NamedNode(p)) = (&s, &p) else {
                bail!("class diagram restriction must identify resources");
            };
            let restriction = sources.node(s).await?;
            let target = match &o {
                Term::NamedNode(o) => sources.node(o).await?,
                Term::Literal(value) => Node::literal(value),
                _ => bail!("unsupported diagram endpoint"),
            };
            let group =
                groups.entry(restriction.id.clone()).or_insert_with(|| {
                    vec![Edge {
                        from:      class.clone(),
                        relation:  read::compact(SUBCLASS, sources.prefixes),
                        predicate: Some(SUBCLASS.into()),
                        to:        restriction.clone(),
                    }]
                });
            group.push(Edge {
                from:      restriction,
                relation:  read::compact(p.as_str(), sources.prefixes),
                predicate: Some(p.as_str().into()),
                to:        target,
            });
        }
        Ok(groups.into_values().collect()) // bounded: the rows above.
    }
}
