//! One ordered, titled footer link with a validated website target.
use {
    super::super::{
        super::{markup::html_escape as esc, model::Plan},
        site_base::SiteBases,
        site_link::{NAVIGATION_SCHEMES, SiteLink},
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};

const TITLE: &str = "http://purl.org/dc/terms/title";

pub(super) struct FooterLink {
    order: u32,
    label: String,
    url:   SiteLink,
    /// A bundled icon shown before the label.
    icon:  Option<&'static str>,
}

impl FooterLink {
    pub async fn read(
        plan: &Plan,
        link: &Term,
        bases: &SiteBases,
    ) -> Result<Self> {
        let Term::NamedNode(link) = link else {
            anyhow::bail!("footer link must be an identified resource")
        };
        let label = title(plan, link).await?;
        let url = SiteLink::authored(
            &plan.source.text(link, "url").await?,
            NAVIGATION_SCHEMES,
            "footer link",
            bases,
        )?;
        let icon = match plan.source.optional(link, "iconName").await? {
            None => None,
            Some(Term::Literal(name)) => {
                Some(
                    super::super::project::icon_svg(name.value()).ok_or_else(
                        || {
                            anyhow::anyhow!(
                                "unsupported footer link icon {}",
                                name.value()
                            )
                        },
                    )?,
                )
            },
            Some(_) => anyhow::bail!("footer link icon must be a literal"),
        };
        Ok(Self {
            order: order(plan, link, "footer link").await?,
            label,
            url,
            icon,
        })
    }

    pub fn order(&self) -> u32 { self.order }

    pub fn html(&self, prefix: &str) -> String {
        format!(
            "<li><a href=\"{}\">{}{}</a></li>",
            esc(&self.url.href(prefix)),
            self.icon
                .map(|svg| {
                    format!("<span class=\"site-footer-icon\">{svg}</span>")
                })
                .unwrap_or_default(),
            esc(&self.label)
        )
    }
}

/// A required, non-empty `dct:title`.
pub(super) async fn title(plan: &Plan, subject: &NamedNode) -> Result<String> {
    let Some(Term::Literal(label)) = plan
        .source
        .optional_by(subject, &NamedNode::new_unchecked(TITLE))
        .await?
    else {
        anyhow::bail!("{subject} requires a dct:title literal");
    };
    ensure!(
        !label.value().trim().is_empty(),
        "{subject} title must not be empty"
    );
    Ok(label.value().to_owned())
}

/// A required nonnegative `ga:order`.
pub(super) async fn order(
    plan: &Plan,
    subject: &NamedNode,
    kind: &str,
) -> Result<u32> {
    crate::publication::validation::unsigned(
        &plan
            .source
            .optional(subject, "order")
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("{kind} {subject} requires ga:order")
            })?,
        &format!("{kind} order"),
    )
}
