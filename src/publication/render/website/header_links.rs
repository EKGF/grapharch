//! Ordered project navigation from the publication's pinned profile.
use {
    super::{
        super::{markup::html_escape, model::Plan},
        site_link::{HEADER_SCHEMES, SiteLink},
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};

pub(super) struct Links(Vec<(u32, String, SiteLink)>);

impl Links {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let mut rows = plan.source.objects(&plan.profile, "headerLink").await?;
        // bounded: a website header permits at most sixteen project links.
        let mut links = Vec::new();
        while let Some(row) = rows.next().await? {
            ensure!(
                links.len() < 16,
                "website header exceeds sixteen project links"
            );
            let Term::NamedNode(link) = &row["value"] else {
                anyhow::bail!("header link must be an identified resource")
            };
            let predicate =
                NamedNode::new_unchecked("http://purl.org/dc/terms/title");
            let Some(Term::Literal(label)) =
                plan.source.optional_by(link, &predicate).await?
            else {
                anyhow::bail!("header link requires a title literal");
            };
            let label = label.value().to_owned();
            ensure!(
                !label.trim().is_empty(),
                "header link title must not be empty"
            );
            let url = SiteLink::authored(
                &plan.source.text(link, "url").await?,
                HEADER_SCHEMES,
                "header link",
            )?;
            let order = crate::publication::validation::unsigned(
                &plan.source.optional(link, "order").await?.ok_or_else(
                    || anyhow::anyhow!("header link requires order"),
                )?,
                "header link order",
            )?;
            ensure!(
                !links.iter().any(|(existing, _, _)| *existing == order),
                "header links require distinct orders"
            );
            links.push((order, label, url));
        }
        links.sort_by_key(|(order, _, _)| *order);
        Ok(Self(links))
    }

    pub fn html(&self, prefix: &str) -> String {
        let mut html = String::new();
        for (_, label, url) in &self.0 {
            html.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                html_escape(&url.href(prefix)),
                html_escape(label)
            ));
        }
        html
    }
}
