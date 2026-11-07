use {
    super::{Diagram, Edge, LANGUAGE, LEGEND, Node},
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    sha2::{Digest, Sha256},
    std::collections::{BTreeMap, HashSet},
};
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";

pub(crate) async fn append(
    dataset: &Dataset,
    view: &RevisionView,
    subject: &NamedNode,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
    body: &mut String,
) -> Result<()> {
    let direct = view.pattern(&format!(r#"
{{ {subject} ?p ?o . BIND({subject} AS ?s) }} UNION
{{ ?s ?p {subject} . BIND({subject} AS ?o) }}
FILTER(?p IN (<{RDFS}subClassOf>, <{OWL}equivalentClass>, <{OWL}disjointWith>, <{RDFS}domain>, <{RDFS}range>))
"#), 41);
    let property = view.pattern(&format!(r#"
?property ?touch {subject} . FILTER(?touch IN (<{RDFS}domain>, <{RDFS}range>))
?property ?p ?o . FILTER(?p IN (<{RDFS}domain>, <{RDFS}range>)) BIND(?property AS ?s)
"#), 42);
    let restrictions = view.pattern(&format!(r#"
{subject} ?relation ?s . FILTER(?relation IN (<{RDFS}subClassOf>, <{OWL}equivalentClass>))
?s a <{OWL}Restriction> ; ?p ?o . FILTER(?p IN (<{OWL}onProperty>, <{OWL}someValuesFrom>, <{OWL}allValuesFrom>, <{OWL}onClass>, <{OWL}onDataRange>, <{OWL}hasValue>, <{OWL}cardinality>, <{OWL}minCardinality>, <{OWL}maxCardinality>, <{OWL}qualifiedCardinality>, <{OWL}minQualifiedCardinality>, <{OWL}maxQualifiedCardinality>))
"#), 43);
    let expressions = view.pattern(&format!(r#"
{subject} ?relation ?s . FILTER(?relation IN (<{RDFS}subClassOf>, <{OWL}equivalentClass>))
?s ?p ?list . FILTER(?p IN (<{OWL}unionOf>, <{OWL}intersectionOf>))
?list <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>*/<http://www.w3.org/1999/02/22-rdf-syntax-ns#first> ?o
"#), 44);
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?s ?p ?o WHERE {{ {{ {direct} }} UNION {{ \
             {property} }} UNION {{ {restrictions} }} UNION {{ {expressions} \
             }} FILTER(isIRI(?s) && (isIRI(?o) || isLiteral(?o))) }} ORDER BY \
             ?s ?p ?o LIMIT 19"
        ))
        .await?;
    let mut edges = Vec::new(); // bounded: eighteen asserted edges plus one overflow sentinel.
    let mut nodes = BTreeMap::new(); // bounded: thirty-nine nodes from the bounded query.
    let focus = node(view, subject, prefixes, documented).await?;
    nodes.insert(subject.clone(), focus.clone());
    let mut overflow = false;
    while let Some(row) = rows.next().await? {
        if edges.len() == 18 {
            overflow = true;
            break;
        }
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
                let mut value = node(view, id, prefixes, documented).await?;
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
        edges.push(Edge {
            from:     nodes[from].clone(),
            relation: if matches!(
                predicate.as_str().strip_prefix(OWL),
                Some("unionOf" | "intersectionOf")
            ) {
                format!("{} member", compact(predicate.as_str(), prefixes))
            } else {
                compact(predicate.as_str(), prefixes)
            },
            to:       target,
        });
    }
    body.push_str("\n\n**Class neighborhood**\n\n");
    body.push_str(LEGEND);
    body.push_str("\n\n");
    if edges.is_empty() {
        write(body, Diagram { focus, edges })?;
        body.push_str(
            "No direct class, domain or range relationships are asserted in \
             the collected sources.\n\n",
        );
    } else {
        for panel in edges.chunks(6) {
            write(body, Diagram {
                focus: focus.clone(),
                edges: panel.to_vec(),
            })?;
        }
    }
    if overflow {
        body.push_str(
            "**Diagram limit:** showing the first 18 asserted relationships \
             in deterministic IRI order. Additional relationships remain in \
             the published source graph; this view is not exhaustive.\n\n",
        );
    }
    Ok(())
}
fn write(body: &mut String, diagram: Diagram) -> Result<()> {
    let source = serde_json::to_string(&diagram)?;
    ensure!(
        source.len() <= 64 * 1024,
        "class diagram exceeds content budget"
    );
    body.push_str(&format!("```{LANGUAGE}\n{source}\n```\n\n"));
    Ok(())
}
async fn node(
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
        let mut count = 0;
        while let Some(row) = types.next().await? {
            count += 1;
            ensure!(count <= 512, "diagram node exceeds type budget");
            if let Term::NamedNode(value) = &row["value"] {
                match value.as_str().strip_prefix(OWL) {
                    Some("Class") => kind = "Class",
                    Some("ObjectProperty") => {
                        kind = "Object property";
                        break;
                    },
                    Some("DatatypeProperty") => {
                        kind = "Datatype property";
                        break;
                    },
                    Some("Restriction") => {
                        kind = "OWL restriction";
                        break;
                    },
                    _ => {},
                }
            }
        }
        kind
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
