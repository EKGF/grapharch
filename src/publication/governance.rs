//! Authority identity and role semantics shared with ekg-ontologies.
use {
    super::{RDF_TYPE, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};
pub const AUTHORITY: &str =
    "https://ekgf.org/ontology/artifact-governance#Authority";
pub const ACRONYM: &str =
    "https://ekgf.org/ontology/artifact-governance#authorityAcronym";
pub const PLAYED_BY: &str =
    "https://www.omg.org/spec/Commons/RolesAndCompositions/isPlayedBy";
pub const W3C_AUTHORITY: &str = "urn:uuid:3129ef76-7e6d-48d4-92c2-4fe98c9621ce";
pub const W3C_ORGANIZATION: &str =
    "urn:uuid:96c6206b-38fb-4625-a784-de7903329897";

pub(crate) async fn name(
    view: &RevisionView,
    role: &NamedNode,
) -> Result<String> {
    ensure!(
        view.has_type(role, "Authority").await?,
        "publisher must be an artifact-governance Authority role"
    );
    let mut types = view.objects_by(role, &NamedNode::new(RDF_TYPE)?).await?;
    while let Some(row) = types.next().await? {
        if let Term::NamedNode(kind) = &row["value"] {
            ensure!(!matches!(kind.as_str(), "https://www.omg.org/spec/Commons/Organizations/Organization" | "https://www.omg.org/spec/Commons/Organizations/LegalEntity" | "https://www.omg.org/spec/Commons/Organizations/OrganizationalSubUnit" | "https://www.omg.org/spec/Commons/PartiesAndSituations/Agent"), "an Authority role and its organization must be distinct resources");
        }
    }
    let mut players =
        view.objects_by(role, &NamedNode::new(PLAYED_BY)?).await?;
    while let Some(row) = players.next().await? {
        let Term::NamedNode(player) = &row["value"] else {
            anyhow::bail!("authority player must be an IRI")
        };
        ensure!(
            player != role,
            "authority role cannot play itself"
        );
    }
    let mut names = view
        .objects_by(
            role,
            &NamedNode::new("http://www.w3.org/2000/01/rdf-schema#label")?,
        )
        .await?;
    let mut selected = Vec::new();
    while let Some(row) = names.next().await? {
        let Term::Literal(label) = &row["value"] else {
            anyhow::bail!("authority label must be a literal")
        };
        if label
            .language()
            .is_none_or(|language| language.eq_ignore_ascii_case("en"))
        {
            selected.push(label.value().to_owned());
        }
    }
    ensure!(
        selected.len() == 1,
        "authority requires exactly one English rdfs:label"
    );
    ensure!(
        !selected[0].is_empty(),
        "authority label cannot be empty"
    );
    Ok(selected.remove(0))
}

pub(crate) async fn acronym(
    view: &RevisionView,
    role: &NamedNode,
) -> Result<String> {
    let mut values = std::collections::BTreeSet::new();
    // These are declared equivalent properties, not a preference/fallback
    // chain.
    for predicate in [NamedNode::new(ACRONYM)?, super::term("shortName")] {
        let mut rows = view.objects_by(role, &predicate).await?;
        while let Some(row) = rows.next().await? {
            let Term::Literal(value) = &row["value"] else {
                anyhow::bail!("authority acronym must be a literal")
            };
            values.insert(value.value().to_owned());
        }
    }
    ensure!(
        values.len() == 1,
        "authority requires one unambiguous authorityAcronym"
    );
    let value = values.pop_first().unwrap();
    ensure!(
        !value.is_empty(),
        "authority acronym cannot be empty"
    );
    Ok(value)
}
