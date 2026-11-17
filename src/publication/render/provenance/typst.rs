//! PDF (Typst) presentation of the provenance appendix.
use {
    super::{
        fields::Value,
        model::{Provenance, Repository},
    },
    crate::publication::render::markup::quote,
    anyhow::Result,
    tokio::io::{AsyncWrite, AsyncWriteExt},
};

impl Provenance {
    pub(in crate::publication::render) async fn typst(
        &self,
        file: &mut (impl AsyncWrite + Unpin),
        preceding_appendices: Option<u32>,
    ) -> Result<()> {
        let mut typst = String::from("#pagebreak()\n#[\n");
        typst.push_str(&match preceding_appendices {
            Some(count) => {
                format!(
                    "#set heading(numbering: \
                     \"A.1\")\n#counter(heading).update({count})\n#\
                     heading(level: 1)[Document provenance]\n"
                )
            },
            None => {
                "#heading(level: 1, numbering: none)[Document provenance]\n"
                    .into()
            },
        });
        typst.push_str(&paragraph(
            "This record identifies the generator and source revisions \
             collected for this publication.",
        ));
        typst.push_str(
            "#ga-table(columns: (auto, 1fr), has-header: false, align: left,\n",
        );
        for field in self.fields() {
            typst.push_str(&format!(
                "  [#strong[#text({})]], [{}],\n",
                quote(field.label),
                field.value.typst()
            ));
        }
        typst.push_str(")\n");
        if self.repositories.is_empty() {
            typst.push_str(&paragraph(
                "No Git repository revision was recorded for these sources.",
            ));
        }
        for repository in &self.repositories {
            typst.push_str(&repository.typst_commits());
        }
        typst.push_str("]\n");
        file.write_all(typst.as_bytes()).await?;
        Ok(())
    }
}

impl Repository {
    fn typst_commits(&self) -> String {
        let mut typst = String::from(
            "#heading(level: 2, numbering: none)[Recent commits]\n",
        );
        typst.push_str(&paragraph(
            "Up to five commits, following the first-parent history from the \
             source commit, with the author of each change. This is not the \
             full contributor list.",
        ));
        for commit in &self.commits {
            typst.push_str(&format!(
                "#block(above: 7pt, breakable: false)[#strong[#text({})] \
                 #text({}) #h(0.4em) {}]\n",
                quote(&commit.author),
                quote(&commit.date()),
                self.commit_value(commit).typst()
            ));
        }
        typst
    }
}

impl Value {
    fn typst(&self) -> String {
        match self {
            Self::Text(text) => format!("#text({})", quote(text)),
            Self::Link { text, url } => {
                format!("#link({})[#text({})]", quote(url), quote(text))
            },
            Self::Commit { sha, url: Some(url) } => {
                format!(
                    "#link({})[#raw({})]",
                    quote(url),
                    quote(sha.short())
                )
            },
            Self::Commit { sha, url: None } => {
                format!("#text(size: 8pt, raw({}))", quote(sha.as_str()))
            },
            Self::Sequence(values) => values.iter().map(Self::typst).collect(),
        }
    }
}

fn paragraph(text: &str) -> String {
    format!("#block(above: 7pt)[#text({})]\n", quote(text))
}
