//! People credited through a target's `ga:credit` rules.
//!
//! A rule names a header role (`ga:creditRole`, such as `ga:editor`) and a
//! source predicate (`ga:creditPredicate`, such as `dc:creator`). Every IRI
//! value of that predicate in the pinned revision is credited in that role.
//! ORCID iD IRIs are normalized to their canonical `https` identity. A GitHub
//! profile is shown as the ORCID person only when that ORCID record lists the
//! profile (`schema:sameAs`). Credited people need a collected or authored
//! `dcterms:title`; a missing name is an error rather than a silent omission.
//! A literal value is a name without an identity: it is listed unlinked, unless
//! a person already listed in the header has exactly that name.
use {
    super::{
        model::Plan,
        people::{Person, family_name},
        specification::metadata::explicit_title,
    },
    crate::publication::{orcid::OrcidId, term},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet},
};

const MAX_RULES: usize = 16;
const MAX_REFERENCES: usize = 256;
/// Identity prefix of a credited name that has no IRI of its own.
pub(super) const NAME_ONLY: &str = "urn:grapharch:credited-name:";

pub(super) async fn derived(
    plan: &Plan,
    role: &str,
    listed: &BTreeSet<NamedNode>,
) -> Result<Vec<Person>> {
    let mut predicates = BTreeSet::new(); // bounded: MAX_RULES rules.
    let mut rules = plan.source.objects(&plan.target, "credit").await?;
    let mut count = 0;
    while let Some(row) = rules.next().await? {
        count += 1;
        ensure!(
            count <= MAX_RULES,
            "target exceeds {MAX_RULES} credit rules"
        );
        let Term::NamedNode(rule) = &row["value"] else {
            bail!("credit rule must be an IRI")
        };
        let credited = plan.source.node(rule, "creditRole").await?;
        ensure!(
            super::people::ROLES
                .iter()
                .any(|known| term(known) == credited),
            "credit rule {rule} names unsupported role {credited}"
        );
        if credited == term(role) {
            predicates.insert(plan.source.node(rule, "creditPredicate").await?);
        }
    }
    let mut people = BTreeMap::new(); // bounded: MAX_REFERENCES identities.
    let mut names = BTreeSet::new(); // bounded: MAX_REFERENCES rows per rule.
    for predicate in predicates {
        let pattern = plan
            .source
            .pattern(&format!("?subject {predicate} ?reference"), 0);
        let mut rows = plan
            .dataset
            .select(format!(
                "SELECT DISTINCT ?reference WHERE {{ {pattern} }} ORDER BY \
                 ?reference LIMIT {}",
                MAX_REFERENCES + 1
            ))
            .await?;
        while let Some(row) = rows.next().await? {
            let reference = match &row["reference"] {
                Term::NamedNode(reference) => reference,
                Term::Literal(name) => {
                    let name = name.value().trim();
                    ensure!(
                        !name.is_empty() && name.len() <= 4096,
                        "credited {predicate} name must be nonempty and \
                         bounded"
                    );
                    names.insert(name.to_owned());
                    continue;
                },
                other => {
                    bail!(
                        "credited {predicate} value {other} must be an IRI or \
                         a name"
                    )
                },
            };
            let (identity, weight) = identity(plan, reference).await?;
            *people.entry(identity).or_insert(0) += weight;
            ensure!(
                people.len() <= MAX_REFERENCES,
                "credited people exceed {MAX_REFERENCES}"
            );
        }
    }
    let mut unnamed = Vec::new(); // bounded: MAX_REFERENCES names.
    let mut known = BTreeSet::new(); // bounded: listed and credited people.
    for id in listed.iter().chain(people.keys()) {
        if !id.as_str().starts_with(NAME_ONLY) {
            known.insert(explicit_title(plan, id).await?);
        }
    }
    for name in names {
        if !known.contains(&name) {
            let id = crate::publication::dataset::identity("credited-name", &[
                &name,
            ]);
            unnamed.push(Person { id, name });
        }
    }
    let mut ranked = Vec::new(); // bounded: MAX_REFERENCES identities.
    for (id, weight) in people {
        if listed.contains(&id) {
            continue;
        }
        let name = explicit_title(plan, &id).await?;
        let surname = family_name(plan, &id, &name).await?;
        ranked.push((
            std::cmp::Reverse(weight),
            surname.to_lowercase(),
            name.to_lowercase(),
            id.to_string(),
            Person { id, name },
        ));
    }
    ranked.sort_by(|left, right| {
        (&left.0, &left.1, &left.2, &left.3)
            .cmp(&(&right.0, &right.1, &right.2, &right.3))
    });
    let mut credited: Vec<Person> =
        ranked.into_iter().map(|entry| entry.4).collect();
    credited.extend(
        unnamed
            .into_iter()
            .filter(|person| !listed.contains(&person.id)),
    );
    Ok(credited)
}

/// The displayed identity of a reference and its recorded commit weight.
async fn identity(
    plan: &Plan,
    reference: &NamedNode,
) -> Result<(NamedNode, u64)> {
    let id = if OrcidId::is_orcid_iri(reference.as_str()) {
        OrcidId::from_iri(reference.as_str())?.canonical()
    } else {
        reference.clone()
    };
    let pattern = plan.source.pattern(
        &format!(
            "?contribution <http://www.w3.org/ns/prov#agent> {id} ; {} ?count",
            term("commitCount")
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?contribution ?count WHERE {{ {pattern} }} LIMIT \
             65"
        ))
        .await?;
    let mut weight = 0u64;
    let mut seen = 0;
    while let Some(row) = rows.next().await? {
        seen += 1;
        ensure!(
            seen <= 64,
            "{id} exceeds 64 repository contributions"
        );
        let Term::Literal(count) = &row["count"] else {
            bail!("commitCount must be an integer")
        };
        weight += count.value().parse::<u64>()?;
    }
    let pattern = plan.source.pattern(
        &format!(
            "?person <https://schema.org/sameAs> {id} ; {} ?orcid",
            term("orcid")
        ),
        0,
    );
    let mut linked = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?person WHERE {{ {pattern} }} LIMIT 2"
        ))
        .await?;
    let Some(row) = linked.next().await? else {
        return Ok((id, weight))
    };
    ensure!(
        linked.next().await?.is_none(),
        "{id} is listed by more than one ORCID record"
    );
    let Term::NamedNode(person) = &row["person"] else {
        bail!("linked ORCID person must be an IRI")
    };
    Ok((person.clone(), weight))
}
