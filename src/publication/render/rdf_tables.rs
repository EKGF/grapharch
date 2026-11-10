//! Shared semantics for the generated RDF predicate/value table projection.
use pulldown_cmark::{Event, Tag, TagEnd};

pub(super) fn matches(events: &[Event<'_>], index: usize) -> bool {
    let Some(header) = events.get(index + 1..index + 9) else {
        return false
    };
    matches!(header, [Event::Start(Tag::TableHead), Event::Start(Tag::TableCell), Event::Text(predicate), Event::End(TagEnd::TableCell), Event::Start(Tag::TableCell), Event::Text(value), Event::End(TagEnd::TableCell), Event::End(TagEnd::TableHead)] if predicate.as_ref() == "RDF predicate" && value.as_ref() == "Value")
}

/// Bound metadata to its immediately following two-column RDF table.
pub(super) fn subjects(
    events: &mut [Event<'_>],
) -> anyhow::Result<
    std::collections::BTreeMap<
        usize,
        crate::publication::table_subject::Subject,
    >,
> {
    use crate::publication::table_subject::{LANGUAGE, Subject};
    let mut subjects = std::collections::BTreeMap::new(); // bounded: one <=1MiB Markdown fragment.
    for index in 0..events.len() {
        if !matches!(&events[index], Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(language))) if language.as_ref()==LANGUAGE)
        {
            continue;
        }
        let Some(Event::Text(source)) = events.get(index + 1) else {
            anyhow::bail!("RDF table subject requires one metadata value")
        };
        let subject = Subject::parse(source)?;
        anyhow::ensure!(
            matches!(
                events.get(index + 2),
                Some(Event::End(TagEnd::CodeBlock))
            ) && matches!(events.get(index+3),Some(Event::Start(Tag::Table(columns))) if columns.len()==2) &&
                matches(events, index + 3),
            "RDF subject metadata must immediately precede its predicate table"
        );
        for event in &mut events[index..index + 3] {
            *event = Event::Text("".into());
        }
        subjects.insert(index + 3, subject);
    }
    Ok(subjects)
}
pub(super) fn subject_html(
    subject: &crate::publication::table_subject::Subject,
) -> String {
    let identifier = super::markup::html_escape(&subject.identifier);
    let iri = super::markup::html_escape(&subject.iri);
    format!(
        "<tr><td colspan=\"2\" \
         style=\"text-align:center\"><code>{identifier}</code></td></\
         tr><tr><td colspan=\"2\" \
         style=\"text-align:center\"><code>{iri}</code></td></tr>"
    )
}
pub(super) fn subject_typst(
    subject: &crate::publication::table_subject::Subject,
) -> String {
    format!(
        "table.cell(colspan: 2, align: center)[#raw({})],table.cell(colspan: \
         2, align: center)[#raw({})],",
        super::markup::quote(&super::typst_markdown::table_text(
            &subject.identifier,
            true
        )),
        super::markup::quote(&super::typst_markdown::table_text(
            &subject.iri,
            true
        ))
    )
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        pulldown_cmark::{Options, Parser},
    };
    #[test]
    fn subject_metadata_requires_its_table_and_spans_preserve_escaped_identity()
    -> anyhow::Result<()> {
        let subject=crate::publication::table_subject::Subject::new("ex:<Party>".into(), "https://example.org/averylongunbrokenidentifierwithmorethan24letters".into())?;
        let marker = subject.markdown()?;
        let mut invalid: Vec<_> =
            Parser::new_ext(&marker, Options::ENABLE_TABLES).collect();
        assert!(subjects(&mut invalid).is_err());
        let html = subject_html(&subject);
        assert!(
            html.contains("ex:&lt;Party&gt;") && !html.contains("ex:<Party>")
        );
        assert!(!html.contains("Identifier:") && !html.contains("IRI:"));
        assert_eq!(html.matches("text-align:center").count(), 2);
        let typst = subject_typst(&subject);
        assert!(!typst.contains("Identifier:") && !typst.contains("IRI:"));
        assert_eq!(typst.matches("align: center").count(), 2);
        assert!(include_str!("typst/theme.typ").contains("y == header-row"));

        assert!(typst.contains("table.cell(colspan: 2,"));
        assert!(
            typst.contains('\u{200b}'),
            "identity must retain table wrapping opportunities"
        );
        Ok(())
    }

    #[test]
    fn only_exact_semantic_header_pair_selects_rdf_layout() {
        for (source, expected) in [
            (
                "| RDF predicate | Value |\n| --- | --- |\n| x | y |",
                true,
            ),
            (
                "| Name | Value |\n| --- | --- |\n| x | y |",
                false,
            ),
            (
                "| RDF predicate | Value | Extra |\n| --- | --- | --- |\n| x \
                 | y | z |",
                false,
            ),
        ] {
            let events: Vec<_> =
                Parser::new_ext(source, Options::ENABLE_TABLES).collect();
            assert_eq!(matches(&events, 0), expected);
        }
    }
}
