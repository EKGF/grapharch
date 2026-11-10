//! Bounded, complete explicit union domains; never a guessed generic domain.
use {
    super::{Edge, Node},
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::Result,
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet, HashSet},
};
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub(super) const UNION: &str = "http://www.w3.org/2002/07/owl#unionOf";
const DOMAIN: &str = "http://www.w3.org/2000/01/rdf-schema#domain";

pub(super) async fn expand(
    dataset: &Dataset,
    view: &RevisionView,
    group: &mut Vec<Edge>,
    nodes: &BTreeMap<NamedNode, Node>,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
) -> Result<Option<String>> {
    let domains: Vec<_> = group
        .iter()
        .filter(|e| e.predicate.as_deref() == Some(DOMAIN))
        .collect(); // bounded: 96 queried facts.
    if domains.len() != 1 || domains[0].to.kind != "OWL expression" {
        return Ok(None);
    }
    let expression = domains[0].to.clone();
    let Some((raw, _)) = nodes.iter().find(|(_, n)| n.id == expression.id)
    else {
        return Ok(None);
    };
    // Only the union and non-semantic display annotations may be elided.
    // Additional class axioms must remain explicit rather than be weakened.
    let pattern = view.pattern(
        &format!(
            r#"
{raw} ?p ?o .
FILTER(!( ?p = <{RDF}type> && ?o = <{OWL}Class> ))
FILTER(?p NOT IN (
 <http://www.w3.org/2000/01/rdf-schema#label>,
 <http://www.w3.org/2000/01/rdf-schema#comment>,
 <http://www.w3.org/2004/02/skos/core#prefLabel>,
 <http://www.w3.org/2004/02/skos/core#definition>,
 <http://purl.org/dc/terms/description>))
"#
        ),
        49,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?p ?o WHERE {{ {pattern} }} LIMIT 3"
        ))
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    if row["p"] != NamedNode::new_unchecked(UNION).into() ||
        rows.next().await?.is_some()
    {
        return Ok(None);
    }
    let Term::NamedNode(mut cell) = row["o"].clone() else {
        return Ok(None);
    };
    drop(rows);
    let mut members = Vec::new(); // bounded: eight complete union members.
    let mut visited = BTreeSet::new(); // bounded: eight list cells.
    let mut member_ids = BTreeSet::new(); // bounded: eight named class references.
    while cell.as_str() != format!("{RDF}nil") {
        if members.len() == 8 || !visited.insert(cell.clone()) {
            return Ok(None);
        }
        let Some(member) = one(view, &cell, &format!("{RDF}first")).await?
        else {
            return Ok(None);
        };
        if member.as_str().starts_with("urn:grapharch:skolem:") ||
            !member_ids.insert(member.clone())
        {
            return Ok(None);
        }
        let node =
            super::read::node(view, &member, prefixes, documented).await?;
        if !matches!(node.kind.as_str(), "Class" | "Resource") {
            return Ok(None);
        }
        members.push(node);
        let Some(next) = one(view, &cell, &format!("{RDF}rest")).await? else {
            return Ok(None);
        };
        cell = next;
    }
    if members.len() < 2 {
        return Ok(None);
    }
    for member in members {
        group.push(Edge {
            from:      expression.clone(),
            predicate: Some(UNION.into()),
            relation:  "owl:unionOf member".into(),
            to:        member,
        });
    }
    Ok(Some(expression.id))
}

async fn one(
    view: &RevisionView,
    subject: &NamedNode,
    predicate: &str,
) -> Result<Option<NamedNode>> {
    let mut rows = view
        .objects_by(subject, &NamedNode::new_unchecked(predicate))
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    let Term::NamedNode(value) = &row["value"] else {
        return Ok(None);
    };
    let value = value.clone();
    Ok(if rows.next().await?.is_none() {
        Some(value)
    } else {
        None
    })
}
