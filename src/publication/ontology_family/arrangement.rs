//! Profile choices for where and with which generated parts a family appears.
use {
    super::{identity, q},
    crate::publication::{validation::unsigned, view::RevisionView},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Quad, Term},
};

/// Section roles of generated family parts that a PDF profile can omit.
pub(crate) const OVERVIEW: &str = "family-overview";
pub(crate) const COVERAGE: &str = "family-coverage";

/// Mark a generated top-level part with its section role.
pub(super) fn role(
    data: &mut Vec<Quad>,
    target: &NamedNode,
    subject: &NamedNode,
    role: &str,
) {
    data.push(q(
        &identity("family-placement", &[
            target.as_str(),
            subject.as_str(),
        ]),
        "sectionRole",
        oxrdf::Literal::new_simple_literal(role),
    ));
}

/// The profile's family arrangement. W3C-style front matter is generated
/// unless the profile turns it off; chapters can be nested under an authored
/// placement. The overview and coverage parts are always generated for the
/// HTML reference; PDF profiles can omit them.
pub(super) struct FamilyArrangement {
    pub front_matter: bool,
    /// The order of the first generated placement.
    pub base:         u32,
    parent:           Option<NamedNode>,
    /// An explicit top-level order for the overview, keeping it first in the
    /// HTML reference when the chapters follow authored front matter.
    overview_order:   Option<u32>,
}

async fn flag(
    view: &RevisionView,
    profile: &NamedNode,
    field: &str,
) -> Result<bool> {
    match view.optional(profile, field).await? {
        None => Ok(true),
        Some(Term::Literal(value)) if value == oxrdf::Literal::from(true) => {
            Ok(true)
        },
        Some(Term::Literal(value)) if value == oxrdf::Literal::from(false) => {
            Ok(false)
        },
        _ => bail!("profile {field} must be true or false"),
    }
}

impl FamilyArrangement {
    pub(super) async fn read(
        view: &RevisionView,
        profile: &NamedNode,
    ) -> Result<Self> {
        let parent = match view.optional(profile, "familyPlacement").await? {
            None => None,
            Some(Term::NamedNode(placement)) => {
                ensure!(
                    view.has_type(&placement, "Placement").await?,
                    "familyPlacement {placement} must be a declared Placement"
                );
                Some(placement)
            },
            Some(_) => bail!("familyPlacement must identify a placement"),
        };
        let base = match (
            view.optional(profile, "familyBaseOrder").await?,
            &parent,
        ) {
            (Some(order), Some(parent)) => {
                let base = unsigned(&order, "familyBaseOrder")?;
                let parent_order = unsigned(
                    &view.optional(parent, "order").await?.ok_or_else(
                        || anyhow::anyhow!("familyPlacement requires an order"),
                    )?,
                    "family placement order",
                )?;
                ensure!(
                    base > parent_order,
                    "familyBaseOrder must follow the family placement's order"
                );
                base
            },
            (None, None) => 2,
            (None, Some(_)) => {
                bail!("familyPlacement requires familyBaseOrder")
            },
            (Some(_), None) => {
                bail!("familyBaseOrder requires familyPlacement")
            },
        };
        let overview_order = view
            .optional(profile, "familyOverviewOrder")
            .await?
            .map(|order| unsigned(&order, "familyOverviewOrder"))
            .transpose()?;
        Ok(Self {
            overview_order,
            front_matter: flag(view, profile, "w3cFrontMatter").await?,
            base,
            parent,
        })
    }

    pub(super) fn overview_order(&self) -> u32 {
        self.overview_order.unwrap_or(self.base)
    }

    /// Place the overview under the family placement unless it has its own
    /// top-level order.
    pub(super) fn nest_overview(
        &self,
        data: &mut Vec<Quad>,
        target: &NamedNode,
        subject: &NamedNode,
    ) {
        if self.overview_order.is_none() {
            self.nest(data, target, subject);
        }
    }

    /// The order of the zero-based top-level chapter.
    pub(super) fn chapter(&self, index: usize) -> u32 {
        self.base + 1 + index as u32
    }

    pub(super) fn coverage_order(&self, chapters: usize) -> u32 {
        self.chapter(chapters)
    }

    /// Place a top-level generated part under the authored family placement.
    pub(super) fn nest(
        &self,
        data: &mut Vec<Quad>,
        target: &NamedNode,
        subject: &NamedNode,
    ) {
        if let Some(parent) = &self.parent {
            data.push(q(
                &identity("family-placement", &[
                    target.as_str(),
                    subject.as_str(),
                ]),
                "parentPlacement",
                parent.clone(),
            ));
        }
    }
}
