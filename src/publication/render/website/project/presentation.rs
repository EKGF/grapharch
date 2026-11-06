//! Explicit page presentation metadata, retained in the pinned graph.
use {
    crate::publication::render::model::Plan,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};

#[derive(Default)]
pub(in crate::publication::render::website) struct Presentation {
    pub landing:   bool,
    pub features:  Vec<String>,
    pub resources: Vec<String>,
    pub icons:     BTreeMap<String, (String, String)>,
}
impl Presentation {
    pub async fn read(plan: &Plan, page: &NamedNode) -> Result<Self> {
        let mut value = Self::default();
        if let Some(hero) = plan.source.optional(page, "heroKind").await? {
            let hero = literal(&hero, "hero presentation")?;
            ensure!(
                matches!(hero.as_str(), "landing" | "introduction"),
                "unsupported hero presentation {hero}"
            );
            value.landing = hero == "landing";
        }
        let mut rows = plan.source.objects(page, "featureSection").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                value.features.len() < 32,
                "page exceeds 32 feature sections"
            );
            value
                .features
                .push(literal(&row["value"], "feature section")?);
        }
        let mut rows = plan.source.objects(page, "resourceSection").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                value.resources.len() < 32,
                "page exceeds 32 resource sections"
            );
            let anchor = literal(&row["value"], "resource section")?;
            ensure!(
                !value.features.contains(&anchor),
                "section {anchor} has conflicting presentations"
            );
            value.resources.push(anchor);
        }
        let mut rows = plan.source.objects(page, "cardPresentation").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                value.icons.len() < 128,
                "page exceeds 128 card presentations"
            );
            let Term::NamedNode(binding) = &row["value"] else {
                anyhow::bail!(
                    "card presentation must be an identified resource"
                )
            };
            let anchor = plan.source.text(binding, "headingAnchor").await?;
            let icon = plan.source.text(binding, "iconName").await?;
            let tone = plan.source.text(binding, "iconTone").await?;
            ensure!(
                icon_svg(&icon).is_some(),
                "unsupported card icon {icon}"
            );
            ensure!(
                matches!(tone.as_str(), "primary" | "accent"),
                "unsupported icon tone {tone}"
            );
            ensure!(
                value.icons.insert(anchor.clone(), (icon, tone)).is_none(),
                "duplicate card presentation {anchor}"
            );
        }
        Ok(value)
    }
}
fn literal(value: &Term, name: &str) -> Result<String> {
    let Term::Literal(value) = value else {
        anyhow::bail!("{name} must be a literal")
    };
    Ok(value.value().to_owned())
}
pub(super) fn icon_svg(name: &str) -> Option<&'static str> {
    match name {
        "triangle-alert" => Some(include_str!("icons/triangle-alert.svg")),
        "search" => Some(include_str!("icons/search.svg")),
        "plug" => Some(include_str!("icons/plug.svg")),
        "users" => Some(include_str!("icons/users.svg")),
        "megaphone" => Some(include_str!("icons/megaphone.svg")),
        "layers" => Some(include_str!("icons/layers.svg")),
        "store" => Some(include_str!("icons/store.svg")),
        "cloud" => Some(include_str!("icons/cloud.svg")),
        "rocket" => Some(include_str!("icons/rocket.svg")),
        "folder-open" => Some(include_str!("icons/folder-open.svg")),
        "quote" => Some(include_str!("icons/quote.svg")),
        "package" => Some(include_str!("icons/package.svg")),
        "database" => Some(include_str!("icons/database.svg")),
        "file-box" => Some(include_str!("icons/file-box.svg")),
        "sparkles" => Some(include_str!("icons/sparkles.svg")),
        "vote" => Some(include_str!("icons/vote.svg")),
        "circle-check" => Some(include_str!("icons/circle-check.svg")),
        "network" => Some(include_str!("icons/network.svg")),
        "file-json" => Some(include_str!("icons/file-json.svg")),
        "git-branch" => Some(include_str!("icons/git-branch.svg")),
        "book-open" => Some(include_str!("icons/book-open.svg")),
        "shield" => Some(include_str!("icons/shield.svg")),
        _ => None,
    }
}
