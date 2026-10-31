//! Explicit reused-term selection from already collected, pinned RDF sources.
use {
    super::{Dataset, RevisionView, q, term},
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Quad, Term},
    std::collections::{BTreeMap, BTreeSet, HashSet},
};

pub(super) async fn selected(
    dataset: &Dataset,
    view: &RevisionView,
    scope: &NamedNode,
    owned: &HashSet<NamedNode>,
) -> Result<Vec<NamedNode>> {
    let mut members = view.objects(scope, "include").await?;
    let mut selected = BTreeSet::new(); // bounded: at most 200 explicit selected terms.
    while let Some(row) = members.next().await? {
        let Term::NamedNode(subject) = &row["value"] else {
            anyhow::bail!("selected ontology term must be an IRI")
        };
        if !owned.contains(subject) {
            selected.insert(subject.clone());
        }
        ensure!(
            selected.len() + owned.len() <= 200,
            "ontology specification exceeds 200-term budget"
        );
    }
    for subject in &selected {
        let pattern = view.pattern(&format!("{subject} a ?kind"), 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?kind WHERE {{ {pattern} }} LIMIT 65"
            ))
            .await?;
        let mut categories = BTreeSet::new();
        let mut property = false;
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= 64,
                "selected term exceeds type budget: {subject}"
            );
            let Term::NamedNode(kind) = &row["kind"] else {
                continue
            };
            match kind.as_str() {
                "http://www.w3.org/2002/07/owl#Class" |
                "http://www.w3.org/2000/01/rdf-schema#Class" => {
                    categories.insert("class");
                },
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#Property" => {
                    property = true
                },
                "http://www.w3.org/2002/07/owl#ObjectProperty" => {
                    categories.insert("object property");
                },
                "http://www.w3.org/2002/07/owl#DatatypeProperty" => {
                    categories.insert("datatype property");
                },
                "http://www.w3.org/2002/07/owl#AnnotationProperty" => {
                    categories.insert("annotation property");
                },
                _ => (),
            }
        }
        ensure!(
            !categories.is_empty() || property,
            "selected term {subject} is missing or has no supported \
             class/property type in explicit sources"
        );
        ensure!(
            categories.len() <= 1,
            "selected term {subject} has ambiguous incompatible \
             class/property types"
        );
        ensure!(
            !(property && categories.contains("class")),
            "selected term {subject} is both class and property"
        );
    }
    Ok(selected.into_iter().collect())
}

pub(super) async fn sources(
    dataset: &Dataset,
    view: &RevisionView,
    subjects: &[NamedNode],
) -> Result<BTreeMap<NamedNode, BTreeSet<String>>> {
    if subjects.is_empty() {
        return Ok(BTreeMap::new())
    }
    // bounded: at most 200 selected identities, with the query-byte cap below.
    let values = subjects
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    ensure!(
        values.len() <= 32 * 1024,
        "selected term identities exceed query budget"
    );
    let pattern = view.pattern("?subject a ?kind", 0);
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?subject ?source WHERE {{ VALUES ?subject {{ \
             {values} }} {pattern} GRAPH <{}> {{ ?_attempt0 {} ?source }} }} \
             ORDER BY ?subject ?source",
            crate::publication::dataset::CONTROL,
            term("source")
        ))
        .await?;
    let mut result = BTreeMap::<NamedNode, BTreeSet<String>>::new(); // bounded: 200 selected terms, 64 source identities each.
    let mut bytes = 0;
    while let Some(row) = rows.next().await? {
        let (Term::NamedNode(subject), Term::Literal(source)) =
            (&row["subject"], &row["source"])
        else {
            anyhow::bail!("invalid selected term source provenance")
        };
        let entries = result.entry(subject.clone()).or_default();
        bytes += source.value().len();
        ensure!(
            bytes <= 1024 * 1024,
            "selected term provenance exceeds byte budget"
        );
        entries.insert(source.value().to_owned());
        ensure!(
            entries.len() <= 64,
            "selected term exceeds source provenance budget"
        );
    }
    Ok(result)
}

pub(super) fn attribution(
    target: &NamedNode,
    component: &NamedNode,
    contribution: &NamedNode,
    subject: &NamedNode,
    sources: &BTreeSet<String>,
    order: u32,
) -> Vec<Quad> {
    let fragment =
        crate::publication::dataset::identity("reused-term-attribution", &[
            target.as_str(),
            subject.as_str(),
        ]);
    let namespace = subject
        .as_str()
        .rfind(['#', '/'])
        .map(|at| &subject.as_str()[..=at])
        .unwrap_or(subject.as_str());
    let mut body = format!(
        "**Source namespace:** `{namespace}`\n\n**Definition sources:**\n"
    );
    for source in sources {
        let escaped = source.replace('`', "\\`");
        body.push_str(&format!("\n- `{escaped}`"));
    }
    vec![
        q(&fragment, "contribution", contribution.clone()),
        q(component, "fragment", fragment.clone()),
        q(&fragment, "type", term("Fragment")),
        q(&fragment, "subject", subject.clone()),
        q(
            &fragment,
            "body",
            Literal::new_simple_literal(body),
        ),
        q(
            &fragment,
            "format",
            Literal::new_simple_literal("text/markdown"),
        ),
        q(&fragment, "order", Literal::from(order)),
    ]
}
