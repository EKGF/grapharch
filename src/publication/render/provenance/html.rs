//! HTML presentation of the provenance appendix.
use {
    super::{
        fields::{Field, Value},
        model::{ID, Provenance, Repository},
    },
    crate::publication::render::markup::{html_escape, structural_heading},
    anyhow::Result,
    tokio::io::{AsyncWrite, AsyncWriteExt},
};

impl Provenance {
    pub(in crate::publication::render) async fn html(
        &self,
        file: &mut (impl AsyncWrite + Unpin),
        title: &str,
    ) -> Result<()> {
        self.html_heading(file, title, 2).await
    }

    pub(in crate::publication::render) async fn html_heading(
        &self,
        file: &mut (impl AsyncWrite + Unpin),
        title: &str,
        level: usize,
    ) -> Result<()> {
        let mut html = format!(
            "<section id=\"{ID}\" class=\"document-provenance\">{}<p>This \
             record identifies the generator and the source revisions \
             collected for this publication.</p>",
            structural_heading(level, title, ID),
        );
        html.push_str(&Self::html_table(&self.fields()));
        if self.repositories.is_empty() {
            html.push_str(
                "<p>No Git repository revision was recorded for these \
                 sources.</p>",
            );
        }
        for repository in &self.repositories {
            html.push_str(&repository.html_commits(level + 1));
        }
        html.push_str("</section>");
        file.write_all(html.as_bytes()).await?;
        Ok(())
    }

    fn html_table(fields: &[Field]) -> String {
        let mut html = String::from("<table class=\"provenance-fields\">");
        let mut group = None;
        let mut open = false;
        for field in fields {
            if !open || field.group != group {
                if open {
                    html.push_str("</tbody>");
                }
                html.push_str(match field.group {
                    Some(_) => "<tbody class=\"provenance-source\">",
                    None => "<tbody>",
                });
                group = field.group;
                open = true;
            }
            html.push_str(&format!(
                "<tr><th scope=\"row\">{}</th><td>{}</td></tr>",
                html_escape(field.label),
                field.value.html()
            ));
        }
        if open {
            html.push_str("</tbody>");
        }
        html.push_str("</table>");
        html
    }
}

impl Repository {
    fn html_commits(&self, level: usize) -> String {
        let level = level.min(6);
        let mut html = format!(
            "<h{level}>Recent commits</h{level}><p>Up to five commits, \
             following the first-parent history from the source commit, with \
             the author of each change. This is not the full contributor \
             list.</p><ol class=\"provenance-commits\">"
        );
        for commit in &self.commits {
            html.push_str(&format!(
                "<li><strong>{}</strong> <time datetime=\"{}\">{}</time> \
                 {}</li>",
                html_escape(&commit.author),
                commit.time.to_rfc3339(),
                html_escape(&commit.date()),
                self.commit_value(commit).html()
            ));
        }
        html.push_str("</ol>");
        html
    }
}

impl Value {
    fn html(&self) -> String {
        match self {
            Self::Text(text) => html_escape(text),
            Self::Link { text, url } => {
                format!(
                    "<a href=\"{}\">{}</a>",
                    html_escape(url),
                    html_escape(text)
                )
            },
            Self::Commit { sha, url: Some(url) } => {
                format!(
                    "<a href=\"{}\" title=\"{}\"><code>{}</code></a>",
                    html_escape(url),
                    sha.as_str(),
                    sha.short()
                )
            },
            Self::Commit { sha, url: None } => {
                format!("<code>{}</code>", sha.as_str())
            },
            Self::Sequence(values) => values.iter().map(Self::html).collect(),
        }
    }
}
