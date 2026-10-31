//! One offline syntax engine and palette for HTML and PDF code blocks.
use {
    super::markup::{html_escape, quote},
    anyhow::{Context, Result},
    std::sync::LazyLock,
    syntect::{
        easy::HighlightLines,
        highlighting::{Color, StyleModifier, Theme, ThemeItem},
        parsing::SyntaxSet,
    },
};

// Immutable bundled language assets, independent of publication inputs.
static SPARQL: LazyLock<SyntaxSet> = LazyLock::new(|| {
    syntect::dumps::from_binary(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/sparql.packdump"
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
        ("meta.mapping.key.json string", Color {
            r: 131,
            g: 91,
            b: 11,
            a: 255,
        }),
        ("source.rq variable", Color {
            r: 103,
            g: 52,
            b: 141,
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
        language => language,
    }
}

fn runs(
    source: &str,
    language: &str,
    mut emit: impl FnMut(&str, Color),
) -> Result<()> {
    let language = language.to_ascii_lowercase();
    let language = canonical(&language);
    let syntaxes = if language == "sparql" {
        &*SPARQL
    } else {
        &*typst::text::RAW_SYNTAXES
    };
    let token = if language == "sparql" { "rq" } else { language };
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
