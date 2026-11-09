//! Bounded static search from selected, pinned page bodies.
use {
    super::{
        catalog::{Catalog, Page},
        esc,
    },
    crate::publication::{
        content_blocks::{Boundary, LANGUAGE},
        render::{content::Entries, model::Plan},
    },
    anyhow::{Result, ensure},
    pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd},
    serde::Serialize,
    std::path::Path,
    tokio::io::AsyncWriteExt,
};
const MAX_BYTES: usize = 32 * 1024 * 1024;
#[derive(Serialize)]
struct Record<'a> {
    title: &'a str,
    route: &'a str,
    text:  String,
}
pub(super) async fn write(
    plan: &Plan,
    catalog: &Catalog,
    stage: &Path,
    files: &mut Vec<String>,
) -> Result<()> {
    for name in ["search-index.json", "search.js"] {
        ensure!(
            !files.iter().any(|f| f == name),
            "search artifact collides with publication output: {name}"
        );
    }
    let mut output =
        tokio::fs::File::create(stage.join("search-index.json")).await?;
    output.write_all(b"[").await?;
    let mut entries = Entries::new(plan).await?;
    let mut current: Option<oxrdf::NamedNode> = None;
    let mut body = String::new(); // bounded: one selected page, maximum 1 MiB.
    let mut bytes = 2usize;
    let mut count = 0usize;
    while let Some(entry) = entries.next(plan).await? {
        let owner = catalog.owner(&entry.fragment)?;
        if current.as_ref().is_some_and(|id| id != &owner) {
            let id = current.take().unwrap();
            append(
                &mut output,
                &catalog.pages[id.as_str()],
                &catalog.abbreviations.clean(&body)?,
                &mut bytes,
                &mut count,
            )
            .await?;
            body.clear();
        }
        current = Some(owner);
        body.push_str(&entry.body);
        ensure!(
            body.len() <= 1024 * 1024,
            "search page exceeds 1 MiB content budget"
        );
    }
    if let Some(id) = current {
        append(
            &mut output,
            &catalog.pages[id.as_str()],
            &catalog.abbreviations.clean(&body)?,
            &mut bytes,
            &mut count,
        )
        .await?;
    }
    output.write_all(b"]").await?;
    output.flush().await?;
    tokio::fs::write(stage.join("search.js"), include_str!("search.js"))
        .await?;
    files.extend(["search-index.json".into(), "search.js".into()]);
    Ok(())
}
async fn append(
    output: &mut tokio::fs::File,
    page: &Page,
    body: &str,
    bytes: &mut usize,
    count: &mut usize,
) -> Result<()> {
    if page.asset || page.hidden {
        return Ok(());
    }
    let record = Record {
        title: &page.title,
        route: &page.route,
        text:  text(body)?,
    };
    let data = serde_json::to_vec(&record)?;
    *bytes += data.len() + 1;
    ensure!(
        *bytes <= MAX_BYTES && *count < 10000,
        "search index exceeds 32 MiB or 10000-page budget"
    );
    if *count > 0 {
        output.write_all(b",").await?;
    }
    output.write_all(&data).await?;
    *count += 1;
    Ok(())
}
fn text(body: &str) -> Result<String> {
    let mut result = String::new();
    let mut boundary = false;
    let mut subject = false;
    for event in Parser::new_ext(body, crate::publication::markdown::options())
    {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref name)))
                if name.as_ref() ==
                    crate::publication::table_subject::LANGUAGE =>
            {
                subject = true;
            },
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref name)))
                if name.as_ref() == LANGUAGE =>
            {
                boundary = true
            },
            Event::End(TagEnd::CodeBlock) => {
                boundary = false;
                subject = false;
                result.push(' ');
            },
            Event::Text(value) if subject => {
                let identity =
                    crate::publication::table_subject::Subject::parse(&value)?;
                result.push_str(&identity.identifier);
                result.push(' ');
                result.push_str(&identity.iri);
                result.push(' ');
            },
            Event::Text(value) if boundary => {
                match Boundary::parse(&value)? {
                    Boundary::Alternative { title } |
                    Boundary::Admonition { title, .. } => {
                        result.push_str(&title);
                        result.push(' ');
                    },
                    _ => {},
                }
            },
            Event::Text(value) | Event::Code(value) => result.push_str(&value),
            Event::SoftBreak | Event::HardBreak | Event::End(_) => {
                result.push(' ')
            },
            _ => {},
        }
    }
    Ok(result.split_whitespace().collect::<Vec<_>>().join(" ")) // bounded: one 1 MiB page.
}
pub(super) fn button() -> &'static str {
    "<button type=\"button\" class=\"site-search-open\" aria-label=\"Search \
     documentation\" aria-haspopup=\"dialog\"><svg viewBox=\"0 0 24 24\" \
     aria-hidden=\"true\"><circle cx=\"10.5\" cy=\"10.5\" r=\"6.5\"/><path \
     d=\"m16 16 5 5\"/></svg></button>"
}
pub(super) fn dialog(prefix: &str) -> String {
    format!(
        "<dialog class=\"site-search\" aria-labelledby=\"site-search-title\" \
         data-search-index=\"{}search-index.json\"><div \
         class=\"site-search-heading\"><h2 id=\"site-search-title\">Search \
         documentation</h2><button type=\"button\" \
         class=\"site-search-close\" aria-label=\"Close \
         search\">Close</button></div><label for=\"site-search-query\">Search \
         all pages</label><input id=\"site-search-query\" type=\"search\" \
         maxlength=\"128\" autocomplete=\"off\"><p \
         class=\"site-search-status\" role=\"status\" \
         aria-live=\"polite\">Enter words to search this site.</p><ul \
         class=\"site-search-results\"></ul></dialog><script \
         src=\"{}search.js\" defer></script>",
        esc(prefix),
        esc(prefix)
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn table_subject_metadata_is_searchable_without_json_syntax()
    -> anyhow::Result<()> {
        let subject = crate::publication::table_subject::Subject::new(
            "ex:Party".into(),
            "https://example.org/Party".into(),
        )?;
        let source = format!(
            "{}| RDF predicate | Value |\n| --- | --- |\n| rdfs:label | Party \
             |\n",
            subject.markdown()?
        );
        let text = super::text(&source)?;
        assert!(
            text.contains("ex:Party") &&
                text.contains("https://example.org/Party")
        );
        assert!(!text.contains('{') && !text.contains("\"identifier\""));
        Ok(())
    }
}
