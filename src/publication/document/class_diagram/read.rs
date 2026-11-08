use {
    super::{Diagram, Edge, LANGUAGE, Node, Projection},
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    sha2::{Digest, Sha256},
    std::collections::{BTreeMap, HashSet},
};
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";

pub(crate) struct Views<'a> {
    pub assertions: &'a RevisionView,
    pub metadata:   &'a RevisionView,
    pub language:   &'a str,
}

pub(crate) async fn append(
    dataset: &Dataset,
    views: Views<'_>,
    subject: &NamedNode,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
    body: &mut String,
    renderer: &crate::publication::document::plantuml::LocalRenderer,
) -> Result<()> {
    let view = views.assertions;
    let metadata = views.metadata;
    let direct = view.pattern(&format!(r#"
{{ {subject} ?p ?o . BIND({subject} AS ?s) }} UNION
{{ ?s ?p {subject} . BIND({subject} AS ?o) }}
FILTER(?p IN (<{RDFS}subClassOf>, <{OWL}equivalentClass>, <{OWL}disjointWith>, <{RDFS}domain>, <{RDFS}range>))
BIND(IF(?p IN (<{RDFS}domain>, <{RDFS}range>), ?s, IF(?s = {subject}, ?o, ?s)) AS ?group)
"#), 41);
    let touching = view.pattern(
        &format!(
            r#"
?property ?touch {subject} . FILTER(?touch IN (<{RDFS}domain>, <{RDFS}range>))
"#
        ),
        42,
    );
    let declarations = metadata.pattern(&format!(r#"
?property ?p ?o . FILTER(?p IN (<{RDFS}domain>, <{RDFS}range>)) BIND(?property AS ?s) BIND(?property AS ?group)
"#), 45);
    let property = format!("{touching} {declarations}");
    let restrictions = view.pattern(&format!(r#"
{subject} ?relation ?s . FILTER(?relation IN (<{RDFS}subClassOf>, <{OWL}equivalentClass>))
BIND(?s AS ?group)
?s a <{OWL}Restriction> ; ?p ?o . FILTER(STRSTARTS(STR(?p), "{OWL}"))
"#), 43);
    // One additional authored restriction level, retained in its complete root
    // group. This is bounded syntax inspection, not an inferred graph.
    let nested = view.pattern(
        &format!(
            r#"
{subject} <{RDFS}subClassOf> ?root .
?root a <{OWL}Restriction>; <{OWL}someValuesFrom> ?s .
?s a <{OWL}Restriction>; ?p ?o . FILTER(STRSTARTS(STR(?p), "{OWL}"))
BIND(?root AS ?group)
"#
        ),
        46,
    );
    let expressions = view.pattern(&format!(r#"
{subject} ?relation ?s . FILTER(?relation IN (<{RDFS}subClassOf>, <{OWL}equivalentClass>))
BIND(?s AS ?group)
?s ?p ?list . FILTER(?p IN (<{OWL}unionOf>, <{OWL}intersectionOf>))
?list <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>*/<http://www.w3.org/1999/02/22-rdf-syntax-ns#first> ?o
"#), 44);
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?group ?s ?p ?o WHERE {{ {{ {direct} }} UNION {{ \
             {property} }} UNION {{ {restrictions} }} UNION {{ {nested} }} \
             UNION {{ {expressions} }} FILTER(isIRI(?s) && (isIRI(?o) || \
             isLiteral(?o))) }} ORDER BY ?group ?s ?p ?o LIMIT 97"
        ))
        .await?;
    let mut groups = BTreeMap::<String, Vec<Edge>>::new(); // bounded: ninety-six facts plus one sentinel.
    let mut count = 0;
    let mut nodes = BTreeMap::new(); // bounded: at most 193 endpoint resources.
    let focus = node(metadata, subject, prefixes, documented).await?;
    nodes.insert(subject.clone(), focus.clone());
    let mut overflow = false;
    while let Some(row) = rows.next().await? {
        let group = row["group"].to_string();
        if count == 96 {
            // Never show a group cut short by the query budget.
            groups.remove(&group);
            overflow = true;
            break;
        }
        count += 1;
        let (Term::NamedNode(from), Term::NamedNode(predicate)) =
            (&row["s"], &row["p"])
        else {
            anyhow::bail!("class diagram relationship must identify resources");
        };
        let mut ids = vec![from]; // bounded: two endpoint IDs.
        if let Term::NamedNode(to) = &row["o"] {
            ids.push(to);
        }
        for id in ids {
            if !nodes.contains_key(id) {
                let mut value =
                    node(metadata, id, prefixes, documented).await?;
                if value.kind == "OWL expression" {
                    let ordinal = nodes
                        .values()
                        .filter(|node| node.kind == "OWL expression")
                        .count() +
                        1;
                    value.label = format!("Anonymous OWL expression {ordinal}");
                }
                nodes.insert(id.clone(), value);
            }
        }
        let target = match &row["o"] {
            Term::NamedNode(to) => nodes[to].clone(),
            Term::Literal(value) => {
                Node {
                    id:    value.to_string(),
                    label: value.value().into(),
                    kind:  "Literal value".into(),
                    link:  None,
                }
            },
            _ => anyhow::bail!("unsupported diagram endpoint"),
        };
        groups.entry(group).or_default().push(Edge {
            predicate: Some(predicate.as_str().into()),
            from:      nodes[from].clone(),
            relation:  if matches!(
                predicate.as_str().strip_prefix(OWL),
                Some("unionOf" | "intersectionOf")
            ) {
                format!("{} member", compact(predicate.as_str(), prefixes))
            } else {
                compact(predicate.as_str(), prefixes)
            },
            to:        target,
        });
    }
    let mut edges = Vec::new(); // bounded: eighteen facts in complete relationship groups.
    let mut group_ends = Vec::new(); // bounded: five complete groups.
    for group in groups.into_values() {
        if group_ends.len() == 5 || edges.len() + group.len() > 18 {
            overflow = true;
            continue;
        }
        edges.extend(group);
        group_ends.push(edges.len());
    }
    body.push_str("\n\n");
    let mut diagram = Diagram {
        focus,
        edges,
        shapes: Vec::new(),
        limited: overflow,
    };
    diagram.shapes = super::constraints::collect(
        &diagram, dataset, view, metadata, prefixes, documented,
    )
    .await?;
    super::display::apply(&mut diagram, metadata, views.language).await?;
    let projection = Projection::generate(diagram, renderer).await?;
    write(body, projection)?;
    Ok(())
}
fn write(body: &mut String, projection: Projection) -> Result<()> {
    let source = serde_json::to_string(&projection)?;
    ensure!(
        source.len() <= 512 * 1024,
        "class diagram exceeds content budget"
    );
    body.push_str(&format!("```{LANGUAGE}\n{source}\n```\n\n"));
    Ok(())
}
pub(super) async fn node(
    view: &RevisionView,
    id: &NamedNode,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
) -> Result<Node> {
    let anonymous = id.as_str().starts_with("urn:grapharch:skolem:");
    let kind = if anonymous {
        "OWL expression"
    } else {
        let mut types = view
            .objects_by(
                id,
                &NamedNode::new_unchecked(crate::publication::RDF_TYPE),
            )
            .await?;
        let mut kind = "Resource";
        let mut property_types = 0_u8;
        let mut count = 0;
        while let Some(row) = types.next().await? {
            count += 1;
            ensure!(count <= 512, "diagram node exceeds type budget");
            if let Term::NamedNode(value) = &row["value"] {
                if value.as_str() ==
                    "http://www.w3.org/1999/02/22-rdf-syntax-ns#Property" &&
                    kind == "Resource"
                {
                    kind = "RDF property";
                }
                match value.as_str().strip_prefix(OWL) {
                    Some("Class") => kind = "Class",
                    Some("ObjectProperty") => {
                        property_types |= 1;
                    },
                    Some("AnnotationProperty") => {
                        property_types |= 4;
                    },
                    Some("DatatypeProperty") => {
                        property_types |= 2;
                    },
                    Some("Restriction") => {
                        kind = "OWL restriction";
                        break;
                    },
                    _ => {},
                }
            }
        }
        match property_types {
            0 => kind,
            1 => "Object property",
            2 => "Datatype property",
            4 => "Annotation property",
            // Conflicting specialized declarations never imply datatype-only
            // presentation merely because one type was read first.
            _ => "RDF property",
        }
    };
    let label = if anonymous {
        "Anonymous OWL expression".to_owned()
    } else {
        compact(id.as_str(), prefixes)
    };
    let link = (!anonymous &&
        (documented.contains(id) ||
            id.as_str().starts_with("https://") ||
            id.as_str().starts_with("http://")))
    .then(|| id.as_str().to_owned());
    Ok(Node {
        id: if anonymous {
            format!(
                "expression-{}",
                hex::encode(Sha256::digest(id.as_str().as_bytes()))
            )
        } else {
            id.as_str().into()
        },
        label,
        kind: kind.into(),
        link,
    })
}
fn compact(iri: &str, prefixes: &BTreeMap<String, String>) -> String {
    prefixes
        .iter()
        .filter_map(|(ns, prefix)| {
            iri.strip_prefix(ns)
                .map(|local| (ns.len(), format!("{prefix}:{local}")))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, label)| label)
        .unwrap_or_else(|| {
            for (ns, prefix) in [(OWL, "owl"), (RDFS, "rdfs")] {
                if let Some(local) = iri.strip_prefix(ns) {
                    return format!("{prefix}:{local}");
                }
            }
            iri.rsplit(['#', '/'])
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(iri)
                .into()
        })
}
