//! Shape-scoped constraints retained beside, never merged into, OWL assertions.
use {
    super::{Diagram, Node, read, shorthand::Shorthand},
    crate::publication::{
        dataset::Dataset,
        document::shacl_class_view,
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    serde::{Deserialize, Serialize},
    std::collections::{BTreeMap, HashSet},
};
const SH: &str = "http://www.w3.org/ns/shacl#";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Field {
    pub property:       Node,
    pub property_shape: String,
    pub range:          Option<Node>,
    pub object:         bool,
    pub minimum:        Option<u64>,
    pub maximum:        Option<u64>,
    pub facts:          Vec<(String, String)>, /* bounded: sixty-four
                                                * original constraints. */
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Shape {
    pub class:   Node,
    pub source:  Node,
    pub fields:  Vec<Field>, // bounded: thirty-two fields per shape.
    pub omitted: usize,
}
impl Shape {
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        [&self.class, &self.source].into_iter().chain(
            self.fields.iter().flat_map(|f| {
                std::iter::once(&f.property).chain(f.range.iter())
            }),
        )
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        [&mut self.class, &mut self.source].into_iter().chain(
            self.fields.iter_mut().flat_map(|f| {
                std::iter::once(&mut f.property).chain(f.range.iter_mut())
            }),
        )
    }
}
impl Field {
    pub fn cardinality(&self) -> String {
        if self.minimum.is_none() && self.maximum.is_none() {
            return String::new();
        }
        format!(
            "{}..{}",
            self.minimum.unwrap_or(0),
            self.maximum
                .map(|n| n.to_string())
                .unwrap_or_else(|| "*".into())
        )
    }
}
pub(super) async fn collect(
    diagram: &Diagram,
    dataset: &Dataset,
    view: &RevisionView,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
) -> Result<Vec<Shape>> {
    let presentation = Shorthand::new(diagram);
    let classes: BTreeMap<_, _> = diagram
        .nodes()
        .filter(|n| {
            n.kind == "Class" && presentation.visible.contains(n.id.as_str())
        })
        .map(|n| (n.id.clone(), n.clone()))
        .collect(); // bounded: thirty-seven retained nodes.
    let mut output = Vec::new(); // bounded: thirty-two shape scopes across displayed classes.
    let mut field_count = 0;
    for class in classes.into_values() {
        for shape in shacl_class_view::read::shapes(
            dataset,
            view,
            &NamedNode::new(class.id.clone())?,
        )
        .await?
        {
            ensure!(
                output.len() < 32,
                "combined class diagram exceeds thirty-two shape scopes"
            );
            let source =
                read::node(view, &shape.id, prefixes, documented).await?;
            let mut fields = Vec::new(); // bounded: total 128 simple fields in one diagram.
            for property in &shape.properties {
                let Some((path, range, object, minimum, maximum)) =
                    simple(property)
                else {
                    continue;
                };
                ensure!(
                    field_count < 128,
                    "combined class diagram exceeds 128 simple constraint \
                     fields"
                );
                field_count += 1;
                fields.push(Field {
                    property: read::node(view, path, prefixes, documented)
                        .await?,
                    property_shape: property.id.as_str().into(),
                    range: match range {
                        Some(range) => {
                            Some(
                                read::node(view, range, prefixes, documented)
                                    .await?,
                            )
                        },
                        None => None,
                    },
                    object,
                    minimum,
                    maximum,
                    facts: property
                        .facts
                        .iter()
                        .map(|(p, v)| (p.as_str().into(), v.to_string()))
                        .collect(),
                });
            }
            output.push(Shape {
                class: class.clone(),
                source,
                omitted: shape.properties.len() - fields.len(),
                fields,
            });
        }
    }
    Ok(output)
}
type Simple<'a> = (
    &'a NamedNode,
    Option<&'a NamedNode>,
    bool,
    Option<u64>,
    Option<u64>,
);
fn simple(property: &shacl_class_view::Property) -> Option<Simple<'_>> {
    let [Term::NamedNode(path)] = property.paths.as_slice() else {
        return None;
    };
    if path.as_str().starts_with("urn:grapharch:skolem:") {
        return None;
    }
    if property.facts.iter().any(|(p, _)| {
        matches!(
            p.as_str().strip_prefix(SH),
            Some(
                "or" | "and" |
                    "not" |
                    "xone" |
                    "qualifiedValueShape" |
                    "qualifiedMinCount" |
                    "qualifiedMaxCount" |
                    "node"
            )
        )
    }) {
        return None;
    }
    let mut range = None;
    let mut object = false;
    let mut minimum = None;
    let mut maximum = None;
    for (predicate, value) in &property.facts {
        match predicate.as_str().strip_prefix(SH) {
            Some("class" | "datatype") => {
                if range.is_some() {
                    return None;
                }
                let Term::NamedNode(node) = value else {
                    return None;
                };
                if node.as_str().starts_with("urn:grapharch:skolem:") {
                    return None;
                }
                range = Some(node);
                object = predicate.as_str().ends_with("#class");
            },
            Some("minCount" | "maxCount") => {
                let Term::Literal(value) = value else {
                    return None;
                };
                if value.datatype().as_str() !=
                    "http://www.w3.org/2001/XMLSchema#integer"
                {
                    return None;
                }
                let count = value.value().parse::<u64>().ok()?;
                let slot = if predicate.as_str().ends_with("#minCount") {
                    &mut minimum
                } else {
                    &mut maximum
                };
                if slot.replace(count).is_some() {
                    return None;
                }
            },
            _ => {},
        }
    }
    if minimum.zip(maximum).is_some_and(|(min, max)| min > max) {
        return None;
    }
    Some((path, range, object, minimum, maximum))
}
