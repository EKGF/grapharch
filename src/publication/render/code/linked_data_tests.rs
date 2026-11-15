//! Token-class regressions for the vendored linked-data grammars.
use {
    super::{TokenClass, html, runs, typst},
    syntect::highlighting::Color,
};

/// The colour of every displayed byte, in source order.
struct ColouredSource {
    text:   String,
    colors: Vec<Color>,
    cursor: usize,
}

impl ColouredSource {
    fn highlight(source: &str, language: &str) -> Self {
        let mut text = String::new();
        // bounded: one fixed test snippet, not publication input.
        let mut colors = Vec::new();
        runs(source, language, |run, color| {
            text.push_str(run);
            colors.extend(std::iter::repeat_n(color, run.len()));
        })
        .unwrap();
        assert_eq!(
            text, source,
            "{language} must preserve every character"
        );
        Self { text, colors, cursor: 0 }
    }

    /// Asserts the next occurrence of `lexeme` is coloured as `class`.
    fn expect(&mut self, lexeme: &str, class: TokenClass) -> &mut Self {
        let start = self.cursor +
            self.text[self.cursor..].find(lexeme).unwrap_or_else(|| {
                panic!("{lexeme:?} after {}", self.cursor)
            });
        let end = start + lexeme.len();
        for (offset, color) in self.colors[start..end].iter().enumerate() {
            assert_eq!(
                *color,
                class.color(),
                "{lexeme:?} byte {offset} at {start} must be {class:?}"
            );
        }
        self.cursor = end;
        self
    }
}

const TURTLE: &str = "@prefix sh: <http://www.w3.org/ns/shacl#> .\n\
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>\n\
@base <https://example.org/> .\n\
BASE <https://example.org/base/>\n\
# definition of the entity\n\
example:Account a sh:NodeShape ;\n\
    rdfs:label \"Account\"@en , 'Konto'@de-DE ;\n\
    sh:minCount 1 ; sh:ratio -1.5 ; sh:scale 2e10 ;\n\
    sh:count \"4\"^^xsd:integer ;\n\
    sh:deactivated true , false ;\n\
    sh:in ( example:A :B ) ;\n\
    sh:node [ sh:path _:b0 ] , [] .\n";

#[test]
fn turtle_colours_every_lexical_construct_by_token_class() {
    use TokenClass::*;
    for language in ["turtle", "ttl", "rdf-turtle"] {
        ColouredSource::highlight(TURTLE, language)
            .expect("@prefix", Keyword)
            .expect("sh:", Namespace)
            .expect("<http://www.w3.org/ns/shacl#>", Text)
            .expect(".", Punctuation)
            .expect("PREFIX", Keyword)
            .expect("xsd:", Namespace)
            .expect("@base", Keyword)
            .expect("<https://example.org/>", Text)
            .expect("BASE", Keyword)
            .expect("# definition of the entity", Comment)
            .expect("example:", Namespace)
            .expect("Account", Name)
            .expect("a", Keyword)
            .expect("sh:", Namespace)
            .expect("NodeShape", Name)
            .expect(";", Punctuation)
            .expect("rdfs:", Namespace)
            .expect("label", Name)
            .expect("\"Account\"", Text)
            .expect("@en", LiteralAnnotation)
            .expect(",", Punctuation)
            .expect("'Konto'", Text)
            .expect("@de-DE", LiteralAnnotation)
            .expect("1", Number)
            .expect("-1.5", Number)
            .expect("2e10", Number)
            .expect("\"4\"", Text)
            .expect("^^", LiteralAnnotation)
            .expect("xsd:", Namespace)
            .expect("integer", Name)
            .expect("true", Constant)
            .expect("false", Constant)
            .expect("(", Punctuation)
            .expect("example:", Namespace)
            .expect("A", Name)
            .expect(":", Namespace)
            .expect("B", Name)
            .expect(")", Punctuation)
            .expect("[", Punctuation)
            .expect("_:", BlankNode)
            .expect("b0", BlankNode)
            .expect("]", Punctuation)
            .expect("[]", Punctuation)
            .expect(".", Punctuation);
    }
}

#[test]
fn sparql_and_turtle_share_token_classes() {
    use TokenClass::*;
    ColouredSource::highlight(
        "PREFIX ex: <https://example.org/>\n# find\nSELECT ?s WHERE { ?s a \
         ex:Thing ; ex:size 42 . }\n",
        "sparql",
    )
    .expect("PREFIX", Keyword)
    .expect("ex:", Namespace)
    .expect("<https://example.org/>", Text)
    .expect("# find", Comment)
    .expect("SELECT", Keyword)
    .expect("?s", Name)
    .expect("a", Keyword)
    .expect("ex:", Namespace)
    .expect("Thing", Name)
    .expect("42", Number);
}

#[test]
fn html_and_pdf_carry_the_turtle_token_colours() {
    let rendered_html = html(TURTLE, "turtle").unwrap();
    let rendered_typst = typst(TURTLE, "turtle").unwrap();
    for class in TokenClass::ALL {
        let Color { r, g, b, .. } = class.color();
        let hex = format!("#{r:02x}{g:02x}{b:02x}");
        assert!(
            rendered_html.contains(&format!("color:{hex}\"")),
            "HTML lacks {class:?}"
        );
        assert!(
            rendered_typst.contains(&format!("rgb(\"{hex}\")")),
            "PDF lacks {class:?}"
        );
    }
}

#[test]
fn every_token_class_is_readable_on_the_code_background() {
    fn luminance(color: Color) -> f64 {
        let linear = |component: u8| {
            let value = f64::from(component) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r) +
            0.7152 * linear(color.g) +
            0.0722 * linear(color.b)
    }
    let background = Color { r: 243, g: 246, b: 251, a: 255 };
    for class in TokenClass::ALL {
        let contrast =
            (luminance(background) + 0.05) / (luminance(class.color()) + 0.05);
        assert!(contrast >= 4.5, "{class:?} contrast {contrast}");
    }
}
