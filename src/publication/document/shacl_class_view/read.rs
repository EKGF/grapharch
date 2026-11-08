use {
    super::{Property, SH, Shape},
    crate::publication::{RDF_TYPE, dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};

pub(super) async fn shapes(
    dataset: &Dataset,
    view: &RevisionView,
    class: &NamedNode,
) -> Result<Vec<Shape>> {
    let pattern = view.pattern(
        &format!(
            "?shape <{RDF_TYPE}> <{SH}NodeShape>; <{SH}targetClass> {class}. \
             FILTER(isIRI(?shape) && !STRSTARTS(STR(?shape), \
             'urn:grapharch:skolem:'))"
        ),
        61,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?shape WHERE {{ {pattern} }} ORDER BY ?shape \
             LIMIT 9"
        ))
        .await?;
    let mut shapes = Vec::new(); // bounded: eight exact-target named shapes plus overflow sentinel.
    let mut budget = 0;
    while let Some(row) = rows.next().await? {
        ensure!(
            shapes.len() < 8,
            "class has more than eight SHACL validation views"
        );
        let Term::NamedNode(id) = &row["shape"] else {
            anyhow::bail!("SHACL node shape must have an IRI");
        };
        let source = facts(dataset, view, id, &mut budget).await?;
        let mut properties = Vec::new(); // bounded: thirty-two property shape references.
        for (predicate, value) in &source {
            if predicate.as_str() != format!("{SH}property") {
                continue;
            }
            ensure!(
                properties.len() < 32,
                "SHACL validation view exceeds thirty-two property shapes"
            );
            let Term::NamedNode(property) = value else {
                anyhow::bail!(
                    "SHACL property shape must identify a collected resource"
                );
            };
            let mut facts = facts(dataset, view, property, &mut budget).await?;
            let mut paths = Vec::new(); // bounded: extracted from at most sixty-four facts.
            facts.retain(|(predicate, value)| {
                if predicate.as_str() == format!("{SH}path") {
                    paths.push(value.clone());
                    false
                } else {
                    true
                }
            });
            properties.push(Property { id: property.clone(), paths, facts });
        }
        shapes.push(Shape { id: id.clone(), facts: source, properties });
    }
    Ok(shapes)
}

async fn facts(
    dataset: &Dataset,
    view: &RevisionView,
    subject: &NamedNode,
    bytes: &mut usize,
) -> Result<Vec<(NamedNode, Term)>> {
    let pattern = view.pattern(
        &format!("{subject} ?p ?o . FILTER(STRSTARTS(STR(?p), '{SH}'))"),
        62,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?p ?o WHERE {{ {pattern} }} ORDER BY ?p ?o LIMIT \
             65"
        ))
        .await?;
    let mut facts = Vec::new(); // bounded: sixty-four direct SHACL facts per shape, aggregate 128 KiB.
    while let Some(row) = rows.next().await? {
        ensure!(
            facts.len() < 64,
            "SHACL shape exceeds sixty-four direct constraints"
        );
        let Term::NamedNode(predicate) = &row["p"] else {
            anyhow::bail!("SHACL constraint predicate must be an IRI");
        };
        *bytes += predicate.as_str().len() + row["o"].to_string().len();
        ensure!(
            *bytes <= 128 * 1024,
            "SHACL class views exceed 128 KiB of constraints"
        );
        facts.push((predicate.clone(), row["o"].clone()));
    }
    Ok(facts)
}
