//! One offline syntax engine and palette for HTML and PDF code blocks.
use {
    super::markup::{html_escape, quote},
    anyhow::{Context, Result},
    std::{borrow::Cow, sync::LazyLock},
    syntect::{
        easy::HighlightLines,
        highlighting::{Color, StyleModifier, Theme, ThemeItem},
        parsing::SyntaxSet,
    },
};

// Immutable bundled language assets, independent of publication inputs.
static LINKED_DATA: LazyLock<SyntaxSet> = LazyLock::new(|| {
    syntect::dumps::from_binary(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/linked-data.packdump"
    )))
});
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    let mut theme = typst::text::RAW_THEME.clone();
    // Darken Typst's light palette for legibility on our pale code background.
    for item in &mut theme.scopes {
        if let Some(color) = &mut item.style.foreground {
            color.r = (u16::from(color.r) * 3 / 4) as u8;
            color.g = (u16::from(color.g) * 3 / 4) as u8;
            color.b = (u16::from(color.b) * 3 / 4) as u8;
        }
    }
    for (selector, color) in [
        (
            "string, source.json string, source.rq string, source.ttl string, \
             string.unquoted.iri",
            Color { r: 20, g: 107, b: 54, a: 255 },
        ),
        (
            "constant.numeric, meta.numeric, source.json constant.numeric",
            Color { r: 154, g: 77, b: 0, a: 255 },
        ),
        (
            "constant.language, source.json constant.language",
            Color { r: 0, g: 90, b: 156, a: 255 },
        ),
        ("variable.other.readwrite.prefixed-name", Color {
            r: 0,
            g: 90,
            b: 156,
            a: 255,
        }),
        ("comment", Color { r: 91, g: 101, b: 117, a: 255 }),
        ("meta.mapping.key.json string", Color {
            r: 104,
            g: 54,
            b: 181,
            a: 255,
        }),
        ("source.rq variable", Color {
            r: 104,
            g: 54,
            b: 181,
            a: 255,
        }),
    ] {
        theme.scopes.push(ThemeItem {
            scope: selector.parse().expect("bundled theme selector"),
            style: StyleModifier {
                foreground: Some(color),
                ..StyleModifier::default()
            },
        });
    }
    theme.settings.foreground = Some(Color { r: 24, g: 43, b: 53, a: 255 });
    theme
});

fn canonical(language: &str) -> &str {
    match language {
        "py" => "python",
        "sh" | "shell" => "bash",
        "js" | "node" | "nodejs" => "javascript",
        "ts" => "typescript",
        "jsonld" | "json-ld" => "json",
        "rq" => "sparql",
        "ttl" | "rdf-turtle" => "turtle",
        language => language,
    }
}

fn runs(
    source: &str,
    language: &str,
    mut emit: impl FnMut(&str, Color),
) -> Result<()> {
    // Normalize before splitting coloured runs: tags between CR and LF would
    // otherwise make HTML parse them as two separate line breaks. Both output
    // formats display the same lines; collected source bytes remain unchanged.
    let normalized = if source.contains('\r') {
        Cow::Owned(source.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(source)
    };
    let source = normalized.as_ref();
    let language = language.to_ascii_lowercase();
    let language = canonical(&language);
    let syntaxes = if matches!(language, "sparql" | "turtle") {
        &*LINKED_DATA
    } else {
        &*typst::text::RAW_SYNTAXES
    };
    let token = match language {
        "sparql" => "rq",
        "turtle" => "ttl",
        language => language,
    };
    let Some(syntax) = syntaxes.find_syntax_by_token(token) else {
        emit(source, THEME.settings.foreground.unwrap());
        return Ok(());
    };
    let mut highlighter = HighlightLines::new(syntax, &THEME);
    // Bounded by the validated fragment budget. Keep lexical state across
    // lines; presentation wrapping happens only after the complete source
    // is tokenised.
    for line in source.split_inclusive('\n') {
        for (style, text) in highlighter
            .highlight_line(line.trim_end_matches('\n'), syntaxes)
            .with_context(|| format!("highlighting {language} code"))?
        {
            emit(text, style.foreground);
        }
        if line.ends_with('\n') {
            emit("\n", THEME.settings.foreground.unwrap());
        }
    }
    Ok(())
}

pub(super) fn html(source: &str, language: &str) -> Result<String> {
    let mut body = String::new();
    runs(source, language, |text, color| {
        body.push_str(&format!(
            "<span style=\"color:#{:02x}{:02x}{:02x}\">{}</span>",
            color.r,
            color.g,
            color.b,
            html_escape(text)
        ));
    })?;
    Ok(format!(
        "<pre><code class=\"language-{}\">{body}</code></pre>",
        html_escape(language)
    ))
}

pub(super) fn typst(source: &str, language: &str) -> Result<String> {
    let mut output = String::from("#ga-code((");
    runs(source, language, |text, color| {
        output.push_str(&format!(
            "({}, rgb(\"#{:02x}{:02x}{:02x}\")),",
            quote(text),
            color.r,
            color.g,
            color.b
        ));
    })?;
    output.push_str("))");
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlighted_code_normalizes_display_line_endings_before_styling() {
        let expected = "SELECT ?s WHERE {\n  ?s ?p \"value\" .\n}\n";
        for newline in ["\n", "\r\n", "\r"] {
            let source = expected.replace('\n', newline);
            let rendered = html(&source, "sparql").unwrap();
            let document = scraper::Html::parse_fragment(&rendered);
            let selector = scraper::Selector::parse("code").unwrap();
            let displayed = document
                .select(&selector)
                .next()
                .unwrap()
                .text()
                .collect::<String>();
            assert_eq!(
                displayed, expected,
                "HTML style boundaries must not duplicate line breaks for \
                 {newline:?}"
            );
            let mut pdf_text = String::new();
            runs(&source, "sparql", |text, _| {
                pdf_text.push_str(text)
            })
            .unwrap();
            assert_eq!(
                pdf_text, expected,
                "HTML and PDF share the same displayed source"
            );
        }
    }
    #[test]
    fn all_aliases_use_a_real_grammar_and_preserve_every_character() {
        for (aliases, source) in [
            (
                &["python", "py"][..],
                "def name():\n    return '''東京\n<script>$value & \
                 `code`\n'''\n",
            ),
            (
                &["bash", "sh", "shell"][..],
                "#!/bin/bash\necho \"$value\" # comment\n",
            ),
            (
                &["javascript", "js", "node", "nodejs"][..],
                "const value = `東京 ${name}`;\n",
            ),
            (
                &["typescript", "ts"][..],
                "const value: string = '東京';\n",
            ),
            (
                &["json", "jsonld", "json-ld"][..],
                "{\"@context\": {\"name\": \"https://schema.org/name\"}, \
                 \"active\": true}\n",
            ),
            (
                &["turtle", "ttl", "rdf-turtle"][..],
                "@prefix ex: <https://example.org/> .\n# comment\nex:dataset \
                 a ex:Dataset ; ex:label \"Bønds\"@en ; ex:count 42 .\n",
            ),
            (
                &["sparql", "rq"][..],
                "PREFIX ex: <https://example.org/>\nSELECT ?s $value WHERE { \
                 ?s ex:p \"東京\" . }\n",
            ),
        ] {
            for language in aliases {
                let mut restored = String::new();
                // bounded: colours of one fixed test snippet, not input data.
                let mut colors = std::collections::HashSet::new();
                runs(source, language, |text, color| {
                    restored.push_str(text);
                    colors.insert((color.r, color.g, color.b));
                })
                .unwrap();
                assert_eq!(restored, source, "{language}");
                assert!(
                    colors.len() >= 3,
                    "{language}: no meaningful lexical colouring"
                );
            }
        }
    }
    #[test]
    fn json_keys_and_string_values_have_distinct_accessible_colours() {
        let mut key = None;
        let mut value = None;
        runs(
            "{\"title\": \"UK Bonds\", \"count\": 42}",
            "json",
            |token, color| {
                if token.contains("title") {
                    key = Some(color);
                }
                if token.contains("Bonds") {
                    value = Some(color);
                }
            },
        )
        .unwrap();
        let key = key.expect("JSON key token");
        let value = value.expect("JSON string value token");
        assert!(
            key.b > key.r && key.r > key.g,
            "keys need a saturated purple distinct from green values: {key:?}"
        );
        assert!(
            value.g > value.r && value.g > value.b,
            "string values need a distinct green: {value:?}"
        );
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
        for color in [key, value] {
            let contrast =
                (luminance(background) + 0.05) / (luminance(color) + 0.05);
            assert!(
                contrast >= 4.5,
                "normal code text needs readable contrast: {contrast}"
            );
        }
    }
    #[test]
    fn multiline_lexical_context_survives_line_boundaries() {
        let mut colors = Vec::new();
        runs(
            "\"\"\"first\nsecond\nthird\"\"\"",
            "python",
            |text, color| {
                if text.contains("first") ||
                    text.contains("second") ||
                    text.contains("third")
                {
                    colors.push(color);
                }
            },
        )
        .unwrap();
        assert_eq!(colors.len(), 3);
        assert!(colors.iter().all(|color| color == &colors[0]));
    }
}
