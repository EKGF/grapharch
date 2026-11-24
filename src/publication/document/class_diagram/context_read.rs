//! Read the bounded notation context of one selected neighbourhood.
use {
    super::{
        Diagram,
        Node,
        NodeKind,
        axiom_plan::AxiomPlan,
        context::{
            Context,
            Inverse,
            NEIGHBOUR_AXIOMS,
            PROPERTY_USES,
            PropertyUse,
        },
        expression::{AxiomRelation, Axioms, ClassAxiom},
        restriction::Scope,
        shorthand::{GENERIC_RESOURCE, Shorthand},
        sources::Sources,
    },
    anyhow::Result,
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet},
};
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const DOMAIN: &str = "http://www.w3.org/2000/01/rdf-schema#domain";
const RANGE: &str = "http://www.w3.org/2000/01/rdf-schema#range";
/// Neighbouring classes inspected for their own restrictions.
const NEIGHBOURS: usize = 12;
/// Axioms read per neighbour.
const NEIGHBOUR_LIMIT: usize = 8;
/// Superclasses visited when deciding whether a property is inherited.
const ANCESTORS: usize = 64;

impl Context {
    pub(super) async fn read(
        diagram: &Diagram,
        sources: &Sources<'_>,
    ) -> Result<Self> {
        let mut context = Self {
            axioms:     Self::neighbours(diagram, sources).await?,
            properties: Vec::new(),
            inverses:   Self::inverses(diagram, sources).await?,
        };
        let mut pairs = BTreeSet::new(); // bounded: PROPERTY_USES pairs.
        for restriction in AxiomPlan::new(&diagram.axioms, Scope::Focus)
            .restrictions
            .into_iter()
            .chain(
                AxiomPlan::new(&context.axioms, Scope::Neighbour).restrictions,
            )
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

    /// Restriction axioms of visible neighbouring classes that draw as
    /// members.
    async fn neighbours(
        diagram: &Diagram,
        sources: &Sources<'_>,
    ) -> Result<Vec<ClassAxiom>> {
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
        let mut axioms = Vec::new(); // bounded: NEIGHBOUR_AXIOMS.
        for class in classes {
            let node = diagram
                .nodes()
                .find(|n| n.id == class)
                .cloned()
                .unwrap_or_else(|| unreachable!("selected from the diagram"));
            let read = Axioms::read(
                sources,
                &node,
                &[AxiomRelation::Subclass],
                NEIGHBOUR_LIMIT,
            )
            .await?;
            for axiom in read.axioms {
                // An expression the focus already draws is never drawn twice.
                let shared = diagram.axioms.iter().any(|a| a.id == axiom.id);
                let drawn = !AxiomPlan::new(
                    std::slice::from_ref(&axiom),
                    Scope::Neighbour,
                )
                .restrictions
                .is_empty();
                if shared || !drawn {
                    continue;
                }
                if axioms.len() == NEIGHBOUR_AXIOMS {
                    return Ok(axioms);
                }
                axioms.push(axiom);
            }
        }
        Ok(axioms)
    }

    /// The single declared inverse of each property drawn in the diagram.
    async fn inverses(
        diagram: &Diagram,
        sources: &Sources<'_>,
    ) -> Result<Vec<Inverse>> {
        let properties: BTreeSet<&str> = diagram
            .nodes()
            .filter(|n| {
                matches!(
                    n.kind,
                    NodeKind::ObjectProperty | NodeKind::RdfProperty
                )
            })
            .map(|n| n.id.as_str())
            .take(PROPERTY_USES)
            .collect(); // bounded: PROPERTY_USES properties.
        let mut inverses = Vec::new(); // bounded: PROPERTY_USES.
        for property in properties {
            let id = NamedNode::new(property)?;
            let pattern = sources.metadata.pattern(
                &format!(
                    "{{ {id} <{OWL}inverseOf> ?q }} UNION {{ ?q \
                     <{OWL}inverseOf> {id} }} FILTER(isIRI(?q) && ?q != {id})"
                ),
                49,
            );
            let mut rows = sources
                .dataset
                .select(format!(
                    "SELECT DISTINCT ?q WHERE {{ {pattern} }} LIMIT 2"
                ))
                .await?;
            let mut found = Vec::new(); // bounded: two rows.
            while let Some(row) = rows.next().await? {
                if let Term::NamedNode(q) = &row["q"] {
                    found.push(q.clone());
                }
            }
            drop(rows);
            if let [inverse] = found.as_slice() &&
                !inverse.as_str().starts_with("urn:grapharch:skolem:")
            {
                inverses.push(Inverse {
                    property: property.to_owned(),
                    inverse:  sources.node(inverse).await?,
                });
            }
        }
        Ok(inverses)
    }
}
