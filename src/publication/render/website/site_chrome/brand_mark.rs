//! One organization identity of a website header: an image linking to the
//! organization, with an accessible name and an optional visible wordmark.
use {
    super::{super::super::markup::html_escape as esc, mark_image::MarkImage},
    crate::publication::render::{
        model::Plan,
        website::{
            site_base::SiteBases,
            site_link::{HEADER_SCHEMES, SiteLink},
        },
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

const TITLE: &str = "http://purl.org/dc/terms/title";

#[derive(Clone)]
pub(super) struct BrandMark {
    title:    String,
    url:      SiteLink,
    image:    MarkImage,
    wordmark: Option<String>,
    order:    u32,
}

impl BrandMark {
    pub async fn read(
        plan: &Plan,
        value: &Term,
        bases: &SiteBases,
    ) -> Result<Self> {
        let Term::NamedNode(mark) = value else {
            bail!("brand mark must be an identified resource")
        };
        ensure!(
            plan.source.has_type(mark, "BrandMark").await?,
            "{mark} is not a declared ga:BrandMark"
        );
        let title = Self::literal(plan, mark, &NamedNode::new(TITLE)?)
            .await?
            .ok_or_else(|| {
            anyhow::anyhow!("brand mark {mark} requires dct:title")
        })?;
        let url = SiteLink::authored(
            &plan.source.text(mark, "url").await?,
            HEADER_SCHEMES,
            "brand mark",
            bases,
        )?;
        let image =
            MarkImage::try_new(&plan.source.text(mark, "image").await?, bases)?;
        let wordmark =
            Self::literal(plan, mark, &crate::publication::term("wordmark"))
                .await?;
        let order = match plan.source.optional(mark, "order").await? {
            None => 0,
            Some(value) => {
                crate::publication::validation::unsigned(
                    &value,
                    "brand mark order",
                )?
            },
        };
        Ok(Self { title, url, image, wordmark, order })
    }

    async fn literal(
        plan: &Plan,
        mark: &NamedNode,
        predicate: &NamedNode,
    ) -> Result<Option<String>> {
        match plan.source.optional_by(mark, predicate).await? {
            None => Ok(None),
            Some(Term::Literal(text)) => {
                let text = text.value().trim();
                ensure!(
                    !text.is_empty() && text.len() <= 160,
                    "{mark} {predicate} must be nonempty text of at most 160 \
                     bytes"
                );
                Ok(Some(text.to_owned()))
            },
            Some(_) => bail!("{mark} {predicate} must be a literal"),
        }
    }

    pub fn order(&self) -> u32 { self.order }

    pub fn title(&self) -> &str { &self.title }

    pub fn image(&self) -> &MarkImage { &self.image }

    /// The header's leading organization identity.
    pub fn brand_html(&self, prefix: &str) -> String {
        let (label, wordmark) = match &self.wordmark {
            Some(wordmark) => {
                (
                    format!("{} home", self.title),
                    format!("<span>{}</span>", esc(wordmark)),
                )
            },
            None => (self.title.clone(), String::new()),
        };
        format!(
            "<a class=\"brand-identity\" href=\"{}\" aria-label=\"{}\"><img \
             src=\"{}\" alt=\"\">{wordmark}</a>",
            esc(&self.url.href(prefix)),
            esc(&label),
            esc(&self.image.src(prefix))
        )
    }

    /// A further identity at the end of the header, such as an umbrella
    /// organization's mark.
    pub fn mark_html(&self, prefix: &str) -> String {
        format!(
            "<a class=\"identity-mark\" href=\"{}\"><img src=\"{}\" \
             alt=\"{}\"></a>",
            esc(&self.url.href(prefix)),
            esc(&self.image.src(prefix)),
            esc(&self.title)
        )
    }
}
