//! Source text made inert inside PlantUML names, members and edge labels.
use std::fmt;

/// Text that PlantUML renders literally.
///
/// Every character outside a conservative safe set becomes a numeric
/// character reference (`&#41;`). PlantUML resolves these references in class
/// names, member compartments, rectangles and arrow labels, yet never parses
/// them as syntax: no stereotype (`<<…>>`), generic parameter (`<…>`), link
/// (`[[…]]`), creole markup, quote, line break or preprocessor directive can
/// arise from source data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Escaped(String);
impl Escaped {
    /// One line of literal text.
    pub fn inline(value: &str) -> Self {
        let mut output = String::with_capacity(value.len());
        for character in value.chars() {
            if character.is_alphanumeric() ||
                matches!(character, ' ' | ':' | '-' | '.')
            {
                output.push(character);
            } else {
                output.push_str(&format!("&#{};", u32::from(character)));
            }
        }
        Self(output)
    }

    /// Literal text wrapped at word or identifier boundaries.
    pub fn wrapped(value: &str) -> Self {
        Self(super::label_lines::render(value))
    }
}
impl fmt::Display for Escaped {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrapped(value: &str) -> String { Escaped::wrapped(value).to_string() }

    #[test]
    fn authored_phrase_labels_wrap_at_words() {
        assert_eq!(
            wrapped("legally delegated authority"),
            "legally delegated\\nauthority"
        );
        assert_eq!(
            wrapped("delegates control to"),
            "delegates control to"
        );
    }

    #[test]
    fn long_labels_wrap_without_discarding_identifier_characters() {
        let identifier = wrapped("InformationSensitivityClassification");
        assert!(identifier.contains("\\n"));
        assert_eq!(
            identifier.replace("\\n", ""),
            "InformationSensitivityClassification"
        );
        assert_eq!(wrapped("rdfs:subClassOf"), "rdfs:\\nsubClassOf");
    }

    #[test]
    fn source_text_cannot_inject_plantuml_or_creole_directives() {
        let malicious =
            "\"\n@enduml\n!include https://evil.example/a\n<img:x>[[x]]\\n";
        let encoded = Escaped::inline(malicious).to_string();
        assert!(!encoded.contains('\n'));
        assert!(!encoded.contains('"'));
        assert!(!encoded.contains("!include"));
        assert!(!encoded.contains("<img:"));
        assert!(!encoded.contains("[["));
    }

    #[test]
    fn reserved_characters_never_reach_plantuml_as_syntax() {
        let encoded =
            Escaped::wrapped("a (b) <c> \"d\" «e» ~f \\g").to_string();
        assert_eq!(
            encoded,
            "a &#40;b&#41; &#60;c&#62; &#34;d&#34; &#171;e&#187; &#126;f \
             &#92;g"
        );
        assert!(
            !encoded.contains(['(', ')', '<', '>', '"', '«', '»', '~']),
            "{encoded}"
        );
    }
}
