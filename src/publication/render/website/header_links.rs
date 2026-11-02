//! Ordered project navigation from the publication's pinned profile.
use {
    super::super::{
        markup::html_escape,
        model::{Plan, title},
    },
    anyhow::{Result, ensure},
    oxrdf::Term,
};

pub(super) struct Links(Vec<(u32, String, String)>);

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
            let label = title(&plan.source, link).await?;
            let url = plan.source.text(link, "url").await?;
            ensure!(
                !url.is_empty() && !url.chars().any(char::is_control),
                "header link URL is empty or contains control characters"
            );
            if let Ok(url) = url::Url::parse(&url) {
                ensure!(
                    matches!(url.scheme(), "http" | "https"),
                    "header links must use HTTP(S) or relative URLs"
                );
            } else {
                ensure!(
                    !url.starts_with("//") &&
                        !url.contains('\\') &&
                        !url.split('/').any(|part| part == ".."),
                    "header link relative URL is invalid"
                );
            }
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
            let relative =
                url::Url::parse(url).is_err() && !url.starts_with(['/', '#']);
            let href = if relative {
                format!("{prefix}{url}")
            } else {
                url.clone()
            };
            html.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                html_escape(&href),
                html_escape(label)
            ));
        }
        html
    }
}
