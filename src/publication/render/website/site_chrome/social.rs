//! The profile's social links (`ga:socialLink`), presented as icons by a
//! footer group with `ga:socialLinks`. An icon name such as
//! `fontawesome/brands/github` selects a bundled outline icon by its service.
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
const MAX_LINKS: usize = 16;

/// A service with a bundled icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Service {
    Twitter,
    LinkedIn,
    GitHub,
    YouTube,
}

impl Service {
    /// The service an icon name ends in, such as
    /// `fontawesome/brands/x-twitter`.
    fn of(icon: &str) -> Option<Self> {
        let name = icon.rsplit('/').next()?;
        Some(match name {
            "twitter" | "x-twitter" | "square-twitter" => Self::Twitter,
            "linkedin" | "linkedin-in" | "square-linkedin" => Self::LinkedIn,
            "github" | "github-alt" | "square-github" => Self::GitHub,
            "youtube" | "square-youtube" => Self::YouTube,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Twitter => "Twitter",
            Self::LinkedIn => "LinkedIn",
            Self::GitHub => "GitHub",
            Self::YouTube => "YouTube",
        }
    }

    fn svg(self) -> &'static str {
        let name = match self {
            Self::Twitter => "twitter",
            Self::LinkedIn => "linkedin",
            Self::GitHub => "github",
            Self::YouTube => "youtube",
        };
        super::super::project::icon_svg(name).expect("bundled social icon")
    }
}

struct SocialLink {
    order:   u32,
    url:     SiteLink,
    label:   String,
    service: Option<Service>,
}

/// The profile's social links in order.
pub(in crate::publication::render::website) struct SocialLinks {
    // bounded: at most MAX_LINKS links.
    links: Vec<SocialLink>,
}

impl SocialLinks {
    pub async fn read(plan: &Plan, bases: &SiteBases) -> Result<Self> {
        let mut links = Vec::new();
        let mut rows = plan.source.objects(&plan.profile, "socialLink").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                links.len() < MAX_LINKS,
                "profile exceeds {MAX_LINKS} social links"
            );
            let Term::NamedNode(link) = &row["value"] else {
                anyhow::bail!("social link must be an identified resource")
            };
            links.push(SocialLink::read(plan, link, bases).await?);
        }
        links.sort_by_key(|link| link.order);
        Ok(Self { links })
    }

    pub fn is_empty(&self) -> bool { self.links.is_empty() }

    pub fn html(&self, prefix: &str) -> String {
        self.links
            .iter()
            .map(|link| {
                let content = match link.service {
                    Some(service) => service.svg().to_owned(),
                    None => esc(&link.label),
                };
                format!(
                    "<a class=\"site-footer-social-link\" href=\"{}\" \
                     aria-label=\"{label}\" title=\"{label}\">{content}</a>",
                    esc(&link.url.href(prefix)),
                    label = esc(&link.label)
                )
            })
            .collect()
    }
}

impl SocialLink {
    async fn read(
        plan: &Plan,
        link: &NamedNode,
        bases: &SiteBases,
    ) -> Result<Self> {
        let url = SiteLink::authored(
            &plan.source.text(link, "url").await?,
            NAVIGATION_SCHEMES,
            "social link",
            bases,
        )?;
        let service = match plan.source.optional(link, "iconName").await? {
            Some(Term::Literal(icon)) => Service::of(icon.value()),
            None => None,
            Some(_) => anyhow::bail!("social link icon must be a literal"),
        };
        let title = match plan
            .source
            .optional_by(link, &NamedNode::new_unchecked(TITLE))
            .await?
        {
            Some(Term::Literal(title)) => Some(title.value().to_owned()),
            None => None,
            Some(_) => anyhow::bail!("social link title must be a literal"),
        };
        let label = title
            .or_else(|| service.map(|s| s.label().to_owned()))
            .unwrap_or_else(|| "Social link".to_owned());
        Ok(Self {
            order: super::footer_link::order(plan, link, "social link").await?,
            url,
            label,
            service,
        })
    }
}
