//! One interpretation endnote for the diagrams actually included in a document.
use {
    anyhow::Result,
    pulldown_cmark::{CodeBlockKind, Event, Parser, Tag},
    tokio::{
        fs::File,
        io::{AsyncWriteExt, BufWriter},
    },
};
pub(super) const ID: &str = "grapharch-diagram-note";
pub(super) const ROUTE: &str = "diagram-notes.html";
pub(super) fn contains(body: &str, format: &str) -> bool {
    format == "text/markdown" && Parser::new_ext(body, crate::publication::markdown::options()).any(|event| matches!(event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) if matches!(info.as_ref(), crate::publication::document::class_diagram::LANGUAGE)))
}
pub(super) fn reference_html() -> &'static str {
    "<sup><a role=\"doc-noteref\" aria-label=\"Diagram interpretation note\" \
     href=\"#grapharch-diagram-note\">[1]</a></sup>"
}
pub(super) fn reference_typst() -> &'static str {
    "#if ga-diagram-note [#align(right)[#text(size: \
     7pt)[#link(label(\"grapharch-diagram-note\"))[Diagram note]]]]\n"
}
pub(super) fn html() -> String {
    format!(
        "<section id=\"{ID}\" role=\"doc-endnotes\" \
         aria-labelledby=\"{ID}-title\"><h2 id=\"{ID}-title\">Diagram \
         interpretation</h2><p>{}</p></section>\n",
        super::markup::html_escape(
            crate::publication::document::class_diagram::LEGEND
        )
    )
}
pub(super) async fn append_html(
    file: &mut BufWriter<File>,
    included: bool,
) -> Result<()> {
    if included {
        file.write_all(html().as_bytes()).await?;
    }
    Ok(())
}
pub(super) async fn append_typst(
    file: &mut BufWriter<File>,
    included: bool,
) -> Result<()> {
    if included {
        file.write_all(
            format!(
                "\n#heading(level: 1, numbering: none)[Diagram \
                 interpretation] <{ID}>\n#text({})\n",
                super::markup::quote(
                    crate::publication::document::class_diagram::LEGEND
                )
            )
            .as_bytes(),
        )
        .await?;
    }
    Ok(())
}
