//! Explicit page presentation metadata, retained in the pinned graph.
use {
    crate::publication::render::model::Plan,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};

#[derive(Default)]
pub(in crate::publication::render::website) struct Presentation {
    pub landing:       bool,
    pub explicit_hero: bool,
    pub features:      Vec<String>,
    pub resources:     Vec<String>,
    pub icons:         BTreeMap<String, CardStyle>,
    pub timelines:     Vec<String>,
    /// Sections presented as a call-to-action band.
    pub callouts:      Vec<String>,
    /// Sections whose cards are presented as a term/definition list.
    pub terms:         Vec<String>,
    /// Sections sharing one tighter rhythm with adjacent compact sections.
    pub compacts:      Vec<String>,
    pub statuses:      BTreeMap<String, Marker>,
}

/// A bundled icon in one theme tone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::publication::render::website) struct Marker {
    pub icon: String,
    pub tone: Tone,
}

/// The decorative presentation of one heading-identified card.
pub(in crate::publication::render::website) struct CardStyle {
    pub marker: Marker,
    /// A short status label shown beside the card title.
    pub badge:  Option<String>,
}

/// Theme colours a decorative icon or badge may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::publication::render::website) enum Tone {
    Primary,
    Accent,
    Muted,
}

impl Tone {
    fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "primary" => Self::Primary,
            "accent" => Self::Accent,
            "muted" => Self::Muted,
            _ => anyhow::bail!("unsupported icon tone {value}"),
        })
    }

    pub fn class(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Accent => "accent",
            Self::Muted => "muted",
        }
    }
}

impl Marker {
    async fn read(plan: &Plan, binding: &NamedNode) -> Result<Self> {
        let icon = plan.source.text(binding, "iconName").await?;
        ensure!(
            icon_svg(&icon).is_some(),
            "unsupported card icon {icon}"
        );
        let tone = Tone::parse(&plan.source.text(binding, "iconTone").await?)?;
        Ok(Self { icon, tone })
    }

    /// The icon element, in the classes cards and timelines share.
    pub fn html(&self) -> String {
        format!(
            "<span class=\"project-card-icon project-card-icon-{}\" \
             aria-hidden=\"true\">{}</span>",
            self.tone.class(),
            icon_svg(&self.icon).expect("validated icon")
        )
    }
}

impl CardStyle {
    /// The badge element, when the card declares one.
    pub fn badge_html(&self) -> Option<String> {
        self.badge.as_ref().map(|badge| {
            format!(
                "<span class=\"project-badge project-badge-{}\">{}</span>",
                self.marker.tone.class(),
                crate::publication::render::markup::html_escape(badge)
            )
        })
    }
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
            value.explicit_hero = true;
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
            let marker = Marker::read(plan, binding).await?;
            let badge =
                match plan.source.optional(binding, "badgeLabel").await? {
                    None => None,
                    Some(label) => {
                        let label = literal(&label, "badge label")?;
                        ensure!(
                            !label.trim().is_empty() &&
                                label.chars().count() <= 48 &&
                                !label.chars().any(char::is_control),
                            "badge label must contain 1–48 characters"
                        );
                        Some(label)
                    },
                };
            ensure!(
                value
                    .icons
                    .insert(anchor.clone(), CardStyle { marker, badge })
                    .is_none(),
                "duplicate card presentation {anchor}"
            );
        }
        let mut rows = plan.source.objects(page, "timelineSection").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                value.timelines.len() < 32,
                "page exceeds 32 timeline sections"
            );
            let anchor = literal(&row["value"], "timeline section")?;
            ensure!(
                !value.features.contains(&anchor) &&
                    !value.resources.contains(&anchor),
                "section {anchor} has conflicting presentations"
            );
            value.timelines.push(anchor);
        }
        value.callouts =
            anchors(plan, page, "calloutSection", "callout section").await?;
        value.terms =
            anchors(plan, page, "termSection", "term section").await?;
        for anchor in value.callouts.iter().chain(&value.terms) {
            ensure!(
                !value.features.contains(anchor) &&
                    !value.resources.contains(anchor) &&
                    !value.timelines.contains(anchor),
                "section {anchor} has conflicting presentations"
            );
        }
        if let Some(anchor) = value
            .callouts
            .iter()
            .find(|anchor| value.terms.contains(anchor))
        {
            anyhow::bail!("section {anchor} has conflicting presentations")
        }
        value.compacts =
            anchors(plan, page, "compactSection", "compact section").await?;
        let mut rows = plan.source.objects(page, "timelineStatus").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                value.statuses.len() < 128,
                "page exceeds 128 timeline statuses"
            );
            let Term::NamedNode(binding) = &row["value"] else {
                anyhow::bail!("timeline status must be an identified resource")
            };
            let label = plan.source.text(binding, "statusLabel").await?;
            ensure!(
                !label.trim().is_empty() && label.chars().count() <= 48,
                "timeline status label must contain 1–48 characters"
            );
            ensure!(
                value
                    .statuses
                    .insert(label.clone(), Marker::read(plan, binding).await?)
                    .is_none(),
                "duplicate timeline status {label}"
            );
        }
        Ok(value)
    }
}
/// The heading anchors a page selects for one section presentation.
async fn anchors(
    plan: &Plan,
    page: &NamedNode,
    property: &str,
    name: &str,
) -> Result<Vec<String>> {
    // bounded: at most 32 sections per presentation.
    let mut anchors = Vec::new();
    let mut rows = plan.source.objects(page, property).await?;
    while let Some(row) = rows.next().await? {
        ensure!(anchors.len() < 32, "page exceeds 32 {name}s");
        anchors.push(literal(&row["value"], name)?);
    }
    Ok(anchors)
}

fn literal(value: &Term, name: &str) -> Result<String> {
    let Term::Literal(value) = value else {
        anyhow::bail!("{name} must be a literal")
    };
    Ok(value.value().to_owned())
}
pub(in crate::publication::render::website) fn icon_svg(
    name: &str,
) -> Option<&'static str> {
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
        "archive" => Some(include_str!("icons/archive.svg")),
        "clock" => Some(include_str!("icons/clock.svg")),
        "circle" => Some(include_str!("icons/circle.svg")),
        "github" => Some(include_str!("icons/github.svg")),
        "arrow-right" => Some(include_str!("icons/arrow-right.svg")),
        _ => None,
    }
}
