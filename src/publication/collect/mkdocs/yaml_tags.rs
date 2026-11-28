//! Read the Python YAML tags that Material for MkDocs and pymdownx
//! configurations use as plain data. Nothing named by a tag is imported or
//! executed; an unrecognized tag is dropped with a typed diagnostic.
use {
    super::warning::{ImportWarning, WarningCode},
    serde_yaml::Value,
};

/// A Python callable that a known configuration names by reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PythonReference {
    /// `material.extensions.emoji.to_svg`: inline SVG emoji and icons.
    EmojiToSvg,
    /// `material.extensions.emoji.twemoji`: the Twemoji and icon index.
    EmojiTwemoji,
    /// `pymdownx.superfences.fence_code_format`: keep fence source verbatim.
    FenceCodeFormat,
}
impl PythonReference {
    const ALL: [Self; 3] =
        [Self::EmojiToSvg, Self::EmojiTwemoji, Self::FenceCodeFormat];

    pub(super) const fn path(self) -> &'static str {
        match self {
            Self::EmojiToSvg => "material.extensions.emoji.to_svg",
            Self::EmojiTwemoji => "material.extensions.emoji.twemoji",
            Self::FenceCodeFormat => "pymdownx.superfences.fence_code_format",
        }
    }

    fn named(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|known| known.path() == path)
    }

    fn marker_text(self) -> String { format!("python:{}", self.path()) }

    /// Whether `value` is the data this reference was replaced by.
    pub(super) fn is(self, value: &Value) -> bool {
        value.as_str() == Some(self.marker_text().as_str())
    }
}

/// A configuration whose YAML tags are replaced by plain data before
/// parsing. The YAML parser resolves `!!` shorthands silently, so tags are
/// read from the source text, outside quotes and comments.
pub(super) struct Untagged {
    pub body:     String,
    pub warnings: Vec<ImportWarning>,
}
impl Untagged {
    pub(super) fn read(body: &str) -> Self {
        // bounded: the caller limits the configuration to 128 KiB.
        let mut warnings = Vec::new();
        let mut output = String::with_capacity(body.len());
        for line in body.split_inclusive('\n') {
            match TagToken::find(line) {
                Some(token) => {
                    output.push_str(&line[..token.start]);
                    output.push_str(&token.replacement(&mut warnings));
                    output.push_str(&line[token.end..]);
                },
                None => output.push_str(line),
            }
        }
        Self { body: output, warnings }
    }
}

/// One tag token on a line, by byte range.
struct TagToken<'a> {
    start: usize,
    end:   usize,
    tag:   &'a str,
}
impl<'a> TagToken<'a> {
    fn find(line: &'a str) -> Option<Self> {
        let mut quote = None;
        let mut escaped = false;
        let mut boundary = true;
        for (index, ch) in line.char_indices() {
            if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' && delimiter == '"' {
                    escaped = true;
                } else if ch == delimiter {
                    quote = None;
                }
                continue;
            }
            match ch {
                '#' if boundary => return None,
                '\'' | '"' => quote = Some(ch),
                '!' if boundary => {
                    let end = line[index..]
                        .find(char::is_whitespace)
                        .map_or(line.len(), |offset| index + offset);
                    return Some(Self {
                        start: index,
                        end,
                        tag: &line[index..end],
                    });
                },
                _ => {},
            }
            if !ch.is_whitespace() {
                boundary = matches!(ch, '[' | '{' | ',' | ':') ||
                    (boundary && ch == '-');
            }
        }
        None
    }

    fn replacement(&self, warnings: &mut Vec<ImportWarning>) -> String {
        let name = self.tag.trim_start_matches('!');
        if let Some(reference) = name
            .strip_prefix("python/name:")
            .and_then(PythonReference::named)
        {
            return format!("\"{}\"", reference.marker_text());
        }
        if name == "python/object/apply:pymdownx.slugs.slugify" {
            // The tagged mapping carries only data keywords such as `case`.
            return String::new();
        }
        warnings.push(ImportWarning::new(
            WarningCode::UnknownYamlTag,
            format!(
                "YAML tag {} is not a known Material/pymdownx value; its \
                 value is read as plain data and nothing is executed",
                self.tag
            ),
        ));
        // The tagged node stays inert data: the tag is never resolved.
        String::new()
    }
}

#[cfg(test)]
mod tests;
