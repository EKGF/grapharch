//! Inline Markdown for footer text: text, emphasis, code and validated links.
//! Any other construct is rejected rather than silently dropped.
use {
    super::super::{
        super::markup::html_escape as esc,
        site_base::SiteBases,
        site_link::{NAVIGATION_SCHEMES, SiteLink},
    },
    anyhow::{Result, bail, ensure},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
};

/// Validated inline Markdown, rendered per page so links follow its prefix.
pub(super) struct FooterText {
    source: String,
    bases:  SiteBases,
}

impl FooterText {
    pub fn try_new(source: String, bases: &SiteBases) -> Result<Self> {
        ensure!(
            !source.trim().is_empty(),
            "footer text must not be empty"
        );
        ensure!(
            source.len() <= 2048,
            "footer text exceeds 2048 bytes"
        );
        let text = Self { source, bases: bases.clone() };
        text.html("")?;
        Ok(text)
    }

    pub fn html(&self, prefix: &str) -> Result<String> {
        let mut html = String::new();
        let mut paragraphs = 0usize;
        for event in Parser::new(&self.source) {
            match event {
                Event::Start(Tag::Paragraph) => {
                    paragraphs += 1;
                    ensure!(
                        paragraphs == 1,
                        "footer text must be one paragraph"
                    );
                },
                Event::End(TagEnd::Paragraph) => {},
                Event::Text(text) => html.push_str(&esc(&text)),
                Event::Code(code) => {
                    html.push_str(&format!("<code>{}</code>", esc(&code)))
                },
                Event::SoftBreak => html.push(' '),
                Event::Start(Tag::Emphasis) => html.push_str("<em>"),
                Event::End(TagEnd::Emphasis) => html.push_str("</em>"),
                Event::Start(Tag::Strong) => html.push_str("<strong>"),
                Event::End(TagEnd::Strong) => html.push_str("</strong>"),
                Event::Start(Tag::Link { dest_url, .. }) => {
                    let url = SiteLink::authored(
                        &dest_url,
                        NAVIGATION_SCHEMES,
                        "footer text link",
                        &self.bases,
                    )?;
                    html.push_str(&format!(
                        "<a href=\"{}\">",
                        esc(&url.href(prefix))
                    ));
                },
                Event::End(TagEnd::Link) => html.push_str("</a>"),
                other => bail!("unsupported footer Markdown {other:?}"),
            }
        }
        Ok(html)
    }
}
