//! Shared source-owned contributor ordering for visible metadata and PDFs.
use {
    super::{model::Plan, specification::metadata::explicit_title},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};

pub(super) struct Person {
    pub id:   NamedNode,
    pub name: String,
}

pub(super) async fn read(plan: &Plan, role: &str) -> Result<Vec<Person>> {
    // bounded: publication metadata permits at most 64 people per role.
    let mut people = Vec::new();
    let mut rows = plan.source.objects(&plan.target, role).await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            people.len() < 64,
            "specification exceeds 64 people in one role"
        );
        let Term::NamedNode(id) = &row["value"] else {
            anyhow::bail!("publication person must be an IRI")
        };
        let name = explicit_title(plan, id).await?;
        let surname = family_name(plan, id, &name).await?;
        let order = plan
            .source
            .optional(id, "order")
            .await?
            .map(|value| {
                crate::publication::validation::unsigned(&value, "person order")
            })
            .transpose()?;
        people.push((
            order,
            surname.to_lowercase(),
            name.to_lowercase(),
            id.to_string(),
            Person { id: id.clone(), name },
        ));
    }
    if people.iter().any(|(order, ..)| order.is_some()) {
        ensure!(
            people.iter().all(|(order, ..)| order.is_some()),
            "all people in an ordered role require an explicit order"
        );
        let mut orders = std::collections::HashSet::new();
        ensure!(
            people.iter().all(|(order, ..)| orders.insert(*order)),
            "people in a role require distinct orders"
        );
    }
    people.sort_by(|left, right| {
        (&left.0, &left.1, &left.2, &left.3)
            .cmp(&(&right.0, &right.1, &right.2, &right.3))
    });
    Ok(people
        .into_iter()
        .map(|(_, _, _, _, person)| person)
        .collect())
}

async fn family_name(
    plan: &Plan,
    person: &NamedNode,
    name: &str,
) -> Result<String> {
    // bounded: three established vocabulary spellings of the same name field.
    let mut names = std::collections::BTreeSet::new();
    for predicate in [
        "https://schema.org/familyName",
        "http://schema.org/familyName",
        "http://xmlns.com/foaf/0.1/familyName",
    ] {
        if let Some(value) = plan
            .source
            .optional_by(person, &NamedNode::new_unchecked(predicate))
            .await?
        {
            let Term::Literal(value) = value else {
                anyhow::bail!("person familyName must be a literal")
            };
            ensure!(
                !value.value().trim().is_empty() && value.value().len() <= 4096,
                "person familyName requires a nonempty bounded literal"
            );
            names.insert(value.value().trim().to_owned());
        }
    }
    ensure!(
        names.len() <= 1,
        "person has conflicting familyName annotations"
    );
    if let Some(name) = names.into_iter().next() {
        return Ok(name);
    }
    // Without structured names, use the last word before a comma-separated
    // role. Compound surnames should be supplied explicitly, never inferred
    // from particles.
    Ok(name
        .split(',')
        .next()
        .unwrap_or(name)
        .split_whitespace()
        .next_back()
        .unwrap_or(name)
        .to_owned())
}
