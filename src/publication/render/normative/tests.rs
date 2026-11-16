use {
    super::{
        Conformance,
        keyword::{Keyword, Segment, Segments},
    },
    pulldown_cmark::{Parser, html},
};

fn html(conformance: Conformance, markdown: &str) -> String {
    let mut output = String::new();
    html::push_html(
        &mut output,
        conformance
            .html(Parser::new_ext(
                markdown,
                crate::publication::markdown::options(),
            ))
            .into_iter(),
    );
    output
}

fn keywords(text: &str) -> Vec<Keyword> {
    Segments::new(text)
        .filter_map(|segment| {
            match segment {
                Segment::Keyword(keyword, _) => Some(keyword),
                Segment::Prose(_) => None,
            }
        })
        .collect()
}

#[test]
fn only_standalone_uppercase_keywords_are_recognized() {
    assert_eq!(keywords("A parser MUST reject it."), [
        Keyword::Must
    ]);
    assert!(keywords("A parser must reject it; it Must not.").is_empty());
    assert!(keywords("MUSTARD, MAYBE, OPTIONALLY, NON-OPTIONAL").is_empty());
    assert!(keywords("SHOULD_NOT and ISHALL").is_empty());
    assert_eq!(
        keywords("(MAY), \"SHOULD\"; OPTIONAL. REQUIRED!"),
        [
            Keyword::May,
            Keyword::Should,
            Keyword::Optional,
            Keyword::Required
        ]
    );
}

#[test]
fn multi_word_forms_win_over_their_first_word() {
    assert_eq!(
        keywords("MUST NOT, SHALL NOT, SHOULD NOT, NOT RECOMMENDED"),
        [
            Keyword::MustNot,
            Keyword::ShallNot,
            Keyword::ShouldNot,
            Keyword::NotRecommended
        ]
    );
    assert_eq!(keywords("MUST not"), [Keyword::Must]);
    assert_eq!(keywords("MUST NOTE"), [Keyword::Must]);
    assert_eq!(keywords("NOT allowed, RECOMMENDED"), [
        Keyword::Recommended
    ]);
    assert_eq!(keywords("MUST\nNOT"), [Keyword::MustNot]);
}

#[test]
fn segments_reassemble_the_source_text() {
    let text = "Clients MUST NOT retry; servers MAY.";
    let joined: String = Segments::new(text)
        .map(|segment| {
            match segment {
                Segment::Prose(prose) => prose,
                Segment::Keyword(_, spelling) => spelling,
            }
        })
        .collect();
    assert_eq!(joined, text);
}

#[test]
fn declared_markdown_wraps_keywords_outside_protected_contexts() {
    let output = html(
        Conformance::BCP14,
        "# The MUST heading\n\nIt MUST NOT fail, MAY *pass* and is \
         [OPTIONAL](https://example.org/MUST \"SHOULD\").\nMUST\nNOT \
         straddles a soft break. `MUST` in code; it must not.\n\n![A MUST \
         image](image.png)\n\n```text\nMUST\n```\n",
    );
    assert_eq!(
        output.matches("<em class=\"rfc2119\">").count(),
        4,
        "{output}"
    );
    assert!(output.contains("<em class=\"rfc2119\">MUST NOT</em> fail"));
    assert!(output.contains("<em class=\"rfc2119\">MAY</em> <em>pass</em>"));
    assert!(output.contains(
        "<a href=\"https://example.org/MUST\" title=\"SHOULD\"><em \
         class=\"rfc2119\">OPTIONAL</em></a>"
    ));
    assert!(
        output.contains("<em class=\"rfc2119\">MUST\nNOT</em> straddles"),
        "{output}"
    );
    assert!(
        output.contains("<h1>The MUST heading</h1>"),
        "{output}"
    );
    assert!(output.contains("<code>MUST</code> in code; it must not."));
    assert!(
        output.contains("alt=\"A MUST image\""),
        "{output}"
    );
    assert!(output.contains("<code class=\"language-text\">MUST\n</code>"));
}

#[test]
fn native_markup_with_the_class_is_not_wrapped_again() {
    let output = html(
        Conformance::BCP14,
        "It <em class=\"rfc2119\">MUST</em> stay single.",
    );
    assert_eq!(output.matches("rfc2119").count(), 1, "{output}");
}

#[test]
fn without_a_declaration_nothing_changes() {
    let source = "It MUST NOT fail\nand MAY pass.";
    let output = html(Conformance::default(), source);
    let mut plain = String::new();
    html::push_html(
        &mut plain,
        Parser::new_ext(source, crate::publication::markdown::options()),
    );
    assert_eq!(output, plain);
    assert_eq!(
        Conformance::default().typst("It MUST pass.", false),
        super::super::typst_breaks::Breakable::table("It MUST pass.", false)
            .text()
    );
    assert_eq!(
        Conformance::default().native("MUST"),
        "#emph[MUST]"
    );
}

#[test]
fn declared_typst_styles_keywords_through_the_theme_function() {
    let typst = Conformance::BCP14.typst("It MUST NOT (ever) fail.", false);
    assert!(
        typst.contains("#{ga-normative[MUST NOT]}"),
        "{typst}"
    );
    assert!(
        !typst.contains("#{ga-normative[MUST]}"),
        "{typst}"
    );
    assert_eq!(
        Conformance::BCP14.native("MAY"),
        "#{ga-normative[MAY]}"
    );
}
