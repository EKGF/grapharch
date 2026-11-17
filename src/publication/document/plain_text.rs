//! Source annotation literals are plain text, not Markdown. Their indentation
//! and line wrapping are source formatting; blank lines separate paragraphs.
use super::annotated_term::escape;

/// One plain-text source literal, rendered as inert Markdown.
#[derive(Clone, Copy)]
pub(crate) struct PlainText<'a>(&'a str);

impl<'a> PlainText<'a> {
    pub(crate) fn new(text: &'a str) -> Self { Self(text) }

    /// Source paragraphs, separated by lines that hold only whitespace.
    fn paragraphs(self) -> impl Iterator<Item = Vec<&'a str>> {
        let mut paragraphs = Vec::new(); // bounded: the caller's literal budget.
        let mut current = Vec::new();
        for line in self.0.lines() {
            if line.trim().is_empty() {
                if !current.is_empty() {
                    paragraphs.push(std::mem::take(&mut current));
                }
            } else {
                current.push(line);
            }
        }
        if !current.is_empty() {
            paragraphs.push(current);
        }
        paragraphs.into_iter()
    }

    /// Flowing prose: each paragraph's whitespace is normalised, so tab
    /// indentation never becomes a Markdown code block.
    pub(crate) fn prose(self) -> String {
        self.paragraphs()
            .map(|lines| escape(&lines.join(" ")))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Line-structured text, such as a list of copyright statements followed
    /// by a licence: every source line keeps its own line, without its
    /// indentation.
    pub(crate) fn lines(self) -> String {
        self.paragraphs()
            .map(|lines| {
                lines
                    .into_iter()
                    .map(escape)
                    .collect::<Vec<_>>()
                    .join("\\\n")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::PlainText;

    #[test]
    fn indented_source_paragraphs_stay_prose() {
        let text =
            "\t\tFirst line\r\n\t\tcontinues *here*.\r\n\t\t\r\n\t\tSecond.";
        assert_eq!(
            PlainText::new(text).prose(),
            "First line continues \\*here\\*.\n\nSecond."
        );
        assert_eq!(
            PlainText::new(text).lines(),
            "First line\\\ncontinues \\*here\\*.\n\nSecond."
        );
    }
}
