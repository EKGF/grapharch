//! Class-scoped qualified counts; never a global property range or count.
use {
    super::{
        Diagram,
        Node,
        layout::Layout,
        projection::{label, text},
    },
    std::collections::BTreeMap,
};

pub(super) const SUBCLASS: &str =
    "http://www.w3.org/2000/01/rdf-schema#subClassOf";
pub(super) const ON_PROPERTY: &str = "http://www.w3.org/2002/07/owl#onProperty";
pub(super) const ON_CLASS: &str = "http://www.w3.org/2002/07/owl#onClass";
pub(super) const MINIMUM: &str =
    "http://www.w3.org/2002/07/owl#minQualifiedCardinality";

const ON_DATA_RANGE: &str = "http://www.w3.org/2002/07/owl#onDataRange";

const MAXIMUM: &str = "http://www.w3.org/2002/07/owl#maxQualifiedCardinality";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Minimum,
    Exact,
    Maximum,
}

const EXACT: &str = "http://www.w3.org/2002/07/owl#qualifiedCardinality";

pub(super) struct QualifiedCount<'a> {
    pub class:    &'a Node,
    pub property: &'a Node,
    pub filler:   &'a Node,
    pub datatype: bool,
    count:        u64,
    kind:         Kind,
    pub consumed: Vec<usize>,
}
impl QualifiedCount<'_> {
    pub fn visible(&self) -> bool {
        self.count > 0 || self.kind == Kind::Maximum
    }

    pub fn guarantees_existence(&self) -> bool {
        !self.datatype && self.count > 0 && self.kind != Kind::Maximum
    }

    fn multiplicity(&self) -> String {
        match self.kind {
            Kind::Minimum => format!("{}..*", self.count),
            Kind::Exact => self.count.to_string(),
            Kind::Maximum => format!("0..{}", self.count),
        }
    }

    pub fn members(
        output: &mut String,
        class: &Node,
        counts: &[Self],
        identifiers: &BTreeMap<&str, usize>,
    ) {
        for count in counts
            .iter()
            .filter(|c| c.datatype && c.visible() && c.class.id == class.id)
        {
            output.push_str(&format!(
                "  {{field}} {} : {} [{}] {{qualified}} \
                 [[[./grapharch-node/n{}]]]\n",
                text(&count.property.label),
                text(
                    &count
                        .filler
                        .id
                        .strip_prefix("http://www.w3.org/2001/XMLSchema#")
                        .map(|name| format!("xsd:{name}"))
                        .unwrap_or_else(|| count.filler.label.clone())
                ),
                count.multiplicity(),
                identifiers[count.property.id.as_str()]
            ));
        }
    }

    pub fn write(
        &self,
        output: &mut String,
        identifiers: &BTreeMap<&str, usize>,
        layout: &Layout<'_>,
    ) {
        if !self.visible() || self.datatype {
            return;
        }
        output.push_str(&format!(
            "n{} {} \"{}\" n{} [[./grapharch-node/n{}]] : {}\\n{{qualified}}\n",
            identifiers[self.class.id.as_str()],
            layout.association_arrow(&self.class.id, &self.filler.id),
            self.multiplicity(),
            identifiers[self.filler.id.as_str()],
            identifiers[self.property.id.as_str()],
            label(&self.property.label),
        ));
    }
}

pub(super) fn collect(diagram: &Diagram) -> Vec<QualifiedCount<'_>> {
    diagram
        .edges
        .iter()
        .filter_map(|edge| {
            if edge.predicate.as_deref() != Some(SUBCLASS) ||
                edge.from.kind != "Class" ||
                edge.to.kind != "OWL expression"
            {
                return None;
            }
            let incident: Vec<_> = diagram
                .edges
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    candidate.from.id == edge.to.id ||
                        candidate.to.id == edge.to.id
                })
                .collect(); // bounded: eighteen retained facts.
            // A complete direct subclass restriction only: extra predicates,
            // shared expression nodes or boolean nesting prevent this
            // simplification.
            if incident.len() != 4 {
                return None;
            }
            let target = |predicate| {
                incident
                    .iter()
                    .find(|(_, e)| {
                        e.from.id == edge.to.id &&
                            e.predicate.as_deref() == Some(predicate)
                    })
                    .map(|(_, e)| &e.to)
            };
            let property = target(ON_PROPERTY)?;
            let (filler, datatype) =
                match (target(ON_CLASS), target(ON_DATA_RANGE)) {
                    (Some(filler), None) => (filler, false),
                    (None, Some(filler)) => (filler, true),
                    _ => return None,
                };
            let (kind, count) = [
                (Kind::Minimum, MINIMUM),
                (Kind::Exact, EXACT),
                (Kind::Maximum, MAXIMUM),
            ]
            .into_iter()
            .find_map(|(kind, predicate)| {
                target(predicate).map(|count| (kind, count))
            })?;
            let valid_property = if datatype {
                matches!(
                    property.kind.as_str(),
                    "Datatype property" | "RDF property" | "Resource"
                )
            } else {
                matches!(
                    property.kind.as_str(),
                    "Object property" | "RDF property" | "Resource"
                )
            };
            if !valid_property ||
                !matches!(filler.kind.as_str(), "Class" | "Resource") ||
                count.kind != "Literal value" ||
                filler.id == super::shorthand::GENERIC_RESOURCE
            {
                return None;
            }
            let count = count_value(count)?;
            if kind == Kind::Exact && count == 0 {
                return None;
            }
            Some(QualifiedCount {
                class: &edge.from,
                property,
                filler,
                datatype,
                count,
                kind,
                consumed: incident.iter().map(|(i, _)| *i).collect(),
            }) // bounded: four complete facts.
        })
        .collect() // bounded: eighteen retained facts.
}

/// Parse original typed RDF values, never their presentation labels.
pub(super) fn count_value(node: &Node) -> Option<u64> {
    if node.kind != "Literal value" {
        return None;
    }
    let literal = node.id.parse::<oxrdf::Literal>().ok()?;
    if !matches!(
        literal.datatype().as_str(),
        "http://www.w3.org/2001/XMLSchema#nonNegativeInteger" |
            "http://www.w3.org/2001/XMLSchema#integer"
    ) {
        return None;
    }
    literal.value().parse().ok()
}
