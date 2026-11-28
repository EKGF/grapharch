//! The licence badge of an authored footer (`ga:footerLicense`): a two-part
//! "License | <title>" badge linking the licence.
use {
    super::super::{
        super::{markup::html_escape as esc, model::Plan},
        site_base::SiteBases,
        site_link::{NAVIGATION_SCHEMES, SiteLink},
    },
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
};

pub(super) struct FooterLicense {
    title: String,
    url:   SiteLink,
}

impl FooterLicense {
    pub async fn read(
        plan: &Plan,
        footer: &NamedNode,
        bases: &SiteBases,
    ) -> Result<Option<Self>> {
        let license =
            match plan.source.optional(footer, "footerLicense").await? {
                None => return Ok(None),
                Some(Term::NamedNode(license)) => license,
                Some(_) => {
                    bail!("ga:footerLicense must be an identified resource")
                },
            };
        Ok(Some(Self {
            title: super::footer_link::title(plan, &license).await?,
            url:   SiteLink::authored(
                &plan.source.text(&license, "url").await?,
                NAVIGATION_SCHEMES,
                "footer licence",
                bases,
            )?,
        }))
    }

    pub fn html(&self, prefix: &str) -> String {
        format!(
            "<p><a class=\"license-badge\" href=\"{}\" rel=\"license\" \
             aria-label=\"License: \
             {title}\"><span>License</span><span>{title}</span></a></p>",
            esc(&self.url.href(prefix)),
            title = esc(&self.title)
        )
    }
}
