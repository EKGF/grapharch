use {
    super::{Diagram, Edge, LANGUAGE, Node, NodeKind, Projection},
    crate::publication::{
        dataset::Dataset,
        document::anonymous_order::AnonymousOrder,
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, HashSet},
};
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";

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
    let skolem = "urn:grapharch:skolem:";
    // Anonymous class expressions are read as typed axioms, not as facts.
    let direct = view.pattern(&format!(r#"
{{ {subject} ?p ?o . BIND({subject} AS ?s) }} UNION
{{ ?s ?p {subject} . BIND({subject} AS ?o) }}
FILTER(?p IN (<{RDFS}subClassOf>, <{OWL}equivalentClass>, <{OWL}disjointWith>, <{RDFS}domain>, <{RDFS}range>))
FILTER(?p IN (<{RDFS}domain>, <{RDFS}range>) || !(STRSTARTS(STR(?s), "{skolem}") || STRSTARTS(STR(?o), "{skolem}")))
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
    // Pairwise disjointness of an owl:AllDisjointClasses statement, grouped
    // with the other class like an asserted owl:disjointWith.
    let all_disjoint = view.pattern(
        &format!(
            r#"
?all a <{OWL}AllDisjointClasses> ; <{OWL}members> ?list .
?list <{RDF}rest>*/<{RDF}first> {subject} .
?list <{RDF}rest>*/<{RDF}first> ?o . FILTER(?o != {subject})
BIND({subject} AS ?s) BIND(<{OWL}disjointWith> AS ?p) BIND(?o AS ?group)
"#
        ),
        43,
    );
    // The shared superclass of a disjoint class, so that disjoint siblings
    // can form a generalization set.
    let siblings = view.pattern(
        &format!(
            r#"
{{ {subject} <{OWL}disjointWith> ?s }} UNION {{ ?s <{OWL}disjointWith> {subject} }} UNION
{{ ?all a <{OWL}AllDisjointClasses> ; <{OWL}members> ?list .
  ?list <{RDF}rest>*/<{RDF}first> {subject} . ?list <{RDF}rest>*/<{RDF}first> ?s . FILTER(?s != {subject}) }}
?s <{RDFS}subClassOf> ?o . {subject} <{RDFS}subClassOf> ?o .
BIND(<{RDFS}subClassOf> AS ?p) BIND(?s AS ?group)
"#
        ),
        44,
    );
    let selection = format!(
        "{{ {direct} }} UNION {{ {property} }} UNION {{ {all_disjoint} }} \
         UNION {{ {siblings} }} FILTER(isIRI(?s) && (isIRI(?o) || \
         isLiteral(?o)))"
    );
    let order = AnonymousOrder::new(dataset, view);
    let mut groups = Vec::<Vec<Edge>>::new(); // bounded: ninety-six facts.
    let mut count = 0;
    let mut nodes = BTreeMap::new(); // bounded: at most 193 endpoint resources.
    let mut ids = HashSet::new(); // bounded: the endpoint nodes above.
    let focus = node(metadata, &order, subject, prefixes, documented).await?;
    ids.insert(focus.id.clone());
    nodes.insert(subject.clone(), focus.clone());
    let mut overflow = false;
    let mut rows = Vec::new(); // bounded: ninety-six facts plus one sentinel.
    'groups: for group in order.groups(&selection).await? {
        let mut facts = dataset
            .select(format!(
                "SELECT DISTINCT ?s ?p ?o WHERE {{ {selection} FILTER(?group \
                 = {group}) }} LIMIT {}",
                97 - count
            ))
            .await?;
        let mut group_rows = Vec::new(); // bounded: the remaining fact budget.
        while let Some(row) = facts.next().await? {
            if count == 96 {
                // Never show a group cut short by the fact budget.
                overflow = true;
                break 'groups;
            }
            count += 1;
            group_rows.push([
                row["s"].clone(),
                row["p"].clone(),
                row["o"].clone(),
            ]);
        }
        let index = groups.len();
        rows.extend(
            order
                .sort_statements(group_rows)
                .await?
                .into_iter()
                .map(|[s, p, o]| (index, s, p, o)),
        );
        groups.push(Vec::new());
    }
    for (group, s, p, o) in rows {
        let (Term::NamedNode(from), Term::NamedNode(predicate)) = (&s, &p)
        else {
            anyhow::bail!("class diagram relationship must identify resources");
        };
        let mut endpoints = vec![from]; // bounded: two endpoint IDs.
        if let Term::NamedNode(to) = &o {
            endpoints.push(to);
        }
        for id in endpoints {
            if !nodes.contains_key(id) {
                let mut value =
                    node(metadata, &order, id, prefixes, documented).await?;
                let base = value.id.clone();
                let mut copy = 1;
                while !ids.insert(value.id.clone()) {
                    copy += 1;
                    value.id = format!("{base}-{copy}");
                }
                nodes.insert(id.clone(), value);
            }
        }
        let target = match &o {
            Term::NamedNode(to) => nodes[to].clone(),
            Term::Literal(value) => Node::literal(value),
            _ => anyhow::bail!("unsupported diagram endpoint"),
        };
        groups[group].push(Edge {
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
    let sources = super::sources::Sources {
        dataset,
        assertions: view,
        metadata,
        prefixes,
        documented,
    };
    let axioms = super::expression::Axioms::read(
        &sources,
        &focus,
        &super::expression::AxiomRelation::ALL,
        super::model::AXIOMS,
    )
    .await?;
    overflow |= axioms.limited;
    groups.extend(axioms.fallback);
    let mut edges = Vec::new(); // bounded: eighteen facts in complete relationship groups.
    let mut group_ends = Vec::new(); // bounded: five complete groups.
    let mut unions = HashSet::new(); // bounded: at most five admitted union domains.
    for mut group in groups {
        let union = super::union_domain::expand(
            dataset, metadata, &mut group, &nodes, prefixes, documented,
        )
        .await?;
        if group_ends.len() == 5 || edges.len() + group.len() > 18 {
            overflow = true;
            continue;
        }
        if let Some(union) = union {
            unions.insert(union);
        }
        edges.extend(group);
        group_ends.push(edges.len());
    }
    body.push_str("\n\n");
    let mut diagram = Diagram {
        focus,
        edges,
        shapes: Vec::new(),
        axioms: axioms.axioms,
        limited: overflow,
        context: super::context::Context::default(),
    };
    diagram.number_expressions();
    for node in diagram.nodes_mut() {
        if unions.contains(&node.id) {
            node.kind = NodeKind::OwlUnion;
        }
    }
    diagram.shapes = super::constraints::collect(
        &diagram, dataset, view, metadata, prefixes, documented,
    )
    .await?;
    diagram.context = super::context::Context::read(&diagram, &sources).await?;
    super::expression_label::apply(&mut diagram);
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
    order: &AnonymousOrder<'_>,
    id: &NamedNode,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
) -> Result<Node> {
    let anonymous = id.as_str().starts_with("urn:grapharch:skolem:");
    let mut datatype = !anonymous && super::datatype::is_builtin(id.as_str());
    let kind = if anonymous {
        NodeKind::OwlExpression
    } else {
        let mut types = view
            .objects_by(
                id,
                &NamedNode::new_unchecked(crate::publication::RDF_TYPE),
            )
            .await?;
        let mut kind = NodeKind::Resource;
        let mut property_types = 0_u8;
        let mut count = 0;
        while let Some(row) = types.next().await? {
            count += 1;
            ensure!(count <= 512, "diagram node exceeds type budget");
            if let Term::NamedNode(value) = &row["value"] {
                if value.as_str() ==
                    "http://www.w3.org/1999/02/22-rdf-syntax-ns#Property" &&
                    kind == NodeKind::Resource
                {
                    kind = NodeKind::RdfProperty;
                }
                if value.as_str() == super::datatype::RDFS_DATATYPE {
                    datatype = true;
                }
                match value.as_str().strip_prefix(OWL) {
                    Some("Class") => kind = NodeKind::Class,
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
                        kind = NodeKind::OwlRestriction;
                        break;
                    },
                    _ => {},
                }
            }
        }
        match property_types {
            0 => kind,
            1 => NodeKind::ObjectProperty,
            2 => NodeKind::DatatypeProperty,
            4 => NodeKind::AnnotationProperty,
            // Conflicting specialized declarations never imply datatype-only
            // presentation merely because one type was read first.
            _ => NodeKind::RdfProperty,
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
            format!("expression-{}", order.digest(id).await?)
        } else {
            id.as_str().into()
        },
        label,
        kind,
        link,
        imported: false,
        datatype,
        from: None,
    })
}
pub(super) fn compact(
    iri: &str,
    prefixes: &BTreeMap<String, String>,
) -> String {
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
