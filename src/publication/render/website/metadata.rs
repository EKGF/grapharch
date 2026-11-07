//! Search metadata comes from the pinned page and publication definitions.
use {
    super::{
        super::{markup::html_escape as esc, model::Plan},
        catalog::{Catalog, Page},
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::path::Path,
};

pub(super) struct Metadata {
    base:        Option<url::Url>,
    description: Option<Term>,
}
impl Metadata {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let base =
            match plan.source.optional(&plan.target, "thisVersion").await? {
                None => None,
                Some(Term::NamedNode(value)) => {
                    let url = url::Url::parse(value.as_str())?;
                    ensure!(
                        matches!(url.scheme(), "https" | "http") &&
                            url.host_str().is_some() &&
                            url.username().is_empty() &&
                            url.password().is_none() &&
                            url.query().is_none() &&
                            url.fragment().is_none() &&
                            url.path().ends_with('/'),
                        "website thisVersion must be an absolute HTTP(S) \
                         directory IRI without credentials/query/fragment"
                    );
                    Some(url)
                },
                _ => anyhow::bail!("website thisVersion must be an IRI"),
            };
        let description = plan
            .source
            .optional_by(
                &plan.project,
                &NamedNode::new_unchecked(
                    "http://purl.org/dc/terms/description",
                ),
            )
            .await?;
        Ok(Self { base, description })
    }

    pub async fn head(
        &self,
        plan: &Plan,
        project: &str,
        page: &Page,
    ) -> Result<String> {
        let mut head = format!(
            "<meta property=\"og:type\" content=\"website\"><meta \
             property=\"og:title\" content=\"{}\"><meta \
             property=\"og:site_name\" content=\"{}\">",
            esc(&page.title),
            esc(project)
        );
        if let Some(description) = plan
            .source
            .optional_by(
                &page.id,
                &NamedNode::new_unchecked(
                    "http://purl.org/dc/terms/description",
                ),
            )
            .await?
            .or_else(|| self.description.clone())
        {
            let Term::Literal(value) = description else {
                anyhow::bail!("page description must be a literal")
            };
            ensure!(
                !value.value().trim().is_empty() && value.value().len() <= 2048,
                "page description must contain 1–2048 bytes"
            );
            let value = esc(value.value());
            head.push_str(&format!(
                "<meta name=\"description\" content=\"{value}\"><meta \
                 property=\"og:description\" content=\"{value}\">"
            ));
        }
        if let Some(base) = &self.base {
            let canonical = esc(base.join(&page.route)?.as_str());
            head.push_str(&format!(
                "<link rel=\"canonical\" href=\"{canonical}\"><meta \
                 property=\"og:url\" content=\"{canonical}\">"
            ));
        }
        if page.hidden {
            head.push_str("<meta name=\"robots\" content=\"noindex,follow\">");
        }
        Ok(head)
    }

    pub async fn indexes(
        &self,
        stage: &Path,
        catalog: &Catalog,
        files: &mut Vec<String>,
    ) -> Result<()> {
        let Some(base) = &self.base else {
            return Ok(())
        };
        let mut sitemap = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">");
        // bounded: catalog admits at most 256 pages.
        for page in catalog
            .pages
            .values()
            .filter(|page| !page.asset && !page.hidden)
        {
            sitemap.push_str(&format!(
                "<url><loc>{}</loc></url>",
                esc(base.join(&page.route)?.as_str())
            ));
        }
        sitemap.push_str("</urlset>");
        tokio::fs::write(stage.join("sitemap.xml"), sitemap).await?;
        tokio::fs::write(
            stage.join("robots.txt"),
            format!(
                "User-agent: *\nAllow: /\nSitemap: {}\n",
                base.join("sitemap.xml")?
            ),
        )
        .await?;
        files.extend(["sitemap.xml".into(), "robots.txt".into()]);
        Ok(())
    }
}
