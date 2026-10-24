use regex::Regex;

/// Converts Markdown content to Typst format
pub struct MarkdownToTypstConverter {
    /// The base heading level to use in Typst (default is 1, which maps # to
    /// =)
    heading_base_level: usize,
}

impl MarkdownToTypstConverter {
    /// Create a new converter with default settings
    pub fn new() -> Self { Self { heading_base_level: 1 } }

    /// Create a new converter with a specific heading base level
    ///
    /// The heading_base_level determines how Markdown headings are mapped to
    /// Typst headings:
    /// - heading_base_level = 1: # -> =, ## -> ==, etc.
    /// - heading_base_level = 2: # -> ==, ## -> ===, etc.
    #[allow(dead_code)]
    pub fn with_heading_base_level(heading_base_level: usize) -> Self {
        Self { heading_base_level }
    }

    /// Convert Markdown content to Typst format
    pub fn convert(&self, content: &str) -> String {
        let mut processed = content.to_string();

        // Pre-processing steps
        processed = self.normalize_newlines(&processed);
        processed = self.mark_url_links(&processed);
        processed = self.remove_language_tags(&processed);

        // Main conversion steps
        processed = self.convert_headings(&processed);
        processed = self.convert_links(&processed);
        processed = self.convert_inline_code(&processed);
        processed = self.convert_code_blocks(&processed);
        processed = self.convert_bold_text(&processed);
        processed = self.convert_italic_text(&processed);
        processed = self.convert_lists(&processed);

        // Post-processing steps
        processed = self.fix_newlines(&processed);
        processed = self.convert_urls_in_angle_brackets(&processed);
        processed = self.remove_surrounding_quotes_from_headings(&processed);
        processed = self.remove_backslashes(&processed);
        processed = self.fix_nested_links(&processed);
        processed = self.restore_marked_urls(&processed);

        processed
    }

    /// Normalize newlines to ensure consistent processing
    fn normalize_newlines(&self, content: &str) -> String {
        content.replace("\r\n", "\n").replace("\r", "\n")
    }

    /// Mark URL links where the visible text is also a URL to prevent double
    /// conversion
    fn mark_url_links(&self, content: &str) -> String {
        // Use a different approach without backreferences
        let re =
            Regex::new(r"\[(https?://[^\]]+)\]\(https?://[^)]+\)").unwrap();
        re.replace_all(content, |caps: &regex::Captures| {
            let url = &caps[1];
            format!("[__KEEP_URL__{}]({})", url, url)
        })
        .to_string()
    }

    /// Restore marked URLs back to their original form
    fn restore_marked_urls(&self, content: &str) -> String {
        content.replace("__KEEP_URL__", "")
    }

    /// Remove language tags (e.g., "text"@en)
    fn remove_language_tags(&self, content: &str) -> String {
        let re = Regex::new(r#""([^"]+)"@[a-z]{2}"#).unwrap();
        re.replace_all(content, "$1").to_string()
    }

    /// Convert URLs in angle brackets to proper Typst links
    fn convert_urls_in_angle_brackets(&self, content: &str) -> String {
        let re = Regex::new(r"<(https?://[^>]+)>").unwrap();
        re.replace_all(content, "#link(\"$1\")[\"$1\"]").to_string()
    }

    /// Remove surrounding quotes from headings
    fn remove_surrounding_quotes_from_headings(&self, content: &str) -> String {
        let re = Regex::new(r#"^(=+\s+)"([^"]+)""#).unwrap();
        re.replace_all(content, "$1$2").to_string()
    }

    /// Remove backslashes from text
    fn remove_backslashes(&self, content: &str) -> String {
        let content = content.replace("\\n", "\n");
        let re = Regex::new(r"\\([a-zA-Z])").unwrap();
        let content = re.replace_all(&content, "$1").to_string();
        content.replace("\\", "")
    }

    /// Fix nested links to prevent double conversion
    fn fix_nested_links(&self, content: &str) -> String {
        let mut result = content.to_string();

        // Fix triple-nested links with quotes (the pattern we're seeing in
        // chapter-7)
        let re = Regex::new(
            r###"#link\("#link\("([^"]+)"\)\["([^"]+)"\]"\)\["#link\("([^"]+)"\)\["([^"]+)"\]"\]"###,
        )
        .unwrap();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("#link(\"{}\")[\"{}\"]", &caps[1], &caps[2])
            })
            .to_string();

        // Fix double links with extra closing brackets
        let re = Regex::new(
            r#"#link\("([^"]+)"\)\[#link\("([^"]+)"\)\[([^\]]+)\]\]"#,
        )
        .unwrap();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("#link(\"{}\")[\"{}\"]", &caps[2], &caps[3])
            })
            .to_string();

        // Fix any remaining double links
        let re =
            Regex::new(r#"#link\("([^"]+)"\)\[#link\("([^"]+)"\)\[([^\]]+)\]"#)
                .unwrap();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("#link(\"{}\")[\"{}\"]", &caps[2], &caps[3])
            })
            .to_string();

        result
    }

    /// Convert Markdown headings to Typst headings
    fn convert_headings(&self, content: &str) -> String {
        let mut result = String::new();

        // Process line by line to ensure we only convert headings at the start
        // of a line
        for line in content.lines() {
            let trimmed = line.trim_start();

            if let Some(heading_content) = trimmed.strip_prefix("# ") {
                let heading_level = self.heading_base_level;
                let equals = "=".repeat(heading_level);
                result.push_str(&format!("{} {}\n", equals, heading_content));
            } else if let Some(heading_content) = trimmed.strip_prefix("## ") {
                let heading_level = self.heading_base_level + 1;
                let equals = "=".repeat(heading_level);
                result.push_str(&format!("{} {}\n", equals, heading_content));
            } else if let Some(heading_content) = trimmed.strip_prefix("### ") {
                let heading_level = self.heading_base_level + 2;
                let equals = "=".repeat(heading_level);
                result.push_str(&format!("{} {}\n", equals, heading_content));
            } else if let Some(heading_content) = trimmed.strip_prefix("#### ")
            {
                let heading_level = self.heading_base_level + 3;
                let equals = "=".repeat(heading_level);
                result.push_str(&format!("{} {}\n", equals, heading_content));
            } else if let Some(heading_content) = trimmed.strip_prefix("##### ")
            {
                let heading_level = self.heading_base_level + 4;
                let equals = "=".repeat(heading_level);
                result.push_str(&format!("{} {}\n", equals, heading_content));
            } else if let Some(heading_content) =
                trimmed.strip_prefix("###### ")
            {
                let heading_level = self.heading_base_level + 5;
                let equals = "=".repeat(heading_level);
                result.push_str(&format!("{} {}\n", equals, heading_content));
            } else {
                // Not a heading, keep the line as is
                result.push_str(line);
                result.push('\n');
            }
        }

        // Remove the trailing newline if the original content didn't have one
        if !content.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }

        result
    }

    /// Convert Markdown links to Typst links
    fn convert_links(&self, content: &str) -> String {
        let re = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
        re.replace_all(content, "#link(\"$2\")[$1]").to_string()
    }

    /// Convert Markdown inline code to Typst raw text
    fn convert_inline_code(&self, content: &str) -> String {
        let re_quote = Regex::new(r"`([^`]+)`").unwrap();
        re_quote.replace_all(content, "#raw(\"$1\")").to_string()
    }

    /// Convert Markdown code blocks to Typst raw blocks
    fn convert_code_blocks(&self, content: &str) -> String {
        let re_code_block = Regex::new(r"```([a-z]*)\n([\s\S]*?)```").unwrap();
        re_code_block
            .replace_all(content, |caps: &regex::Captures| {
                let language = &caps[1];
                let code = &caps[2];

                if language.is_empty() {
                    format!("#raw(block: true, \"{}\")", code)
                } else {
                    format!(
                        "#raw(block: true, lang: \"{}\", \"{}\")",
                        language, code
                    )
                }
            })
            .to_string()
    }

    /// Convert Markdown bold text to Typst strong text
    fn convert_bold_text(&self, content: &str) -> String {
        let re_bold = Regex::new(r"\*\*([^*]+)\*\*").unwrap();
        re_bold.replace_all(content, "#strong[$1]").to_string()
    }

    /// Convert Markdown italic text to Typst emphasized text
    fn convert_italic_text(&self, content: &str) -> String {
        let re_italic = Regex::new(r"\*([^*]+)\*").unwrap();
        re_italic.replace_all(content, "#emph[$1]").to_string()
    }

    /// Convert Markdown lists to Typst lists
    fn convert_lists(&self, content: &str) -> String {
        let mut processed = content.to_string();

        // Unordered lists
        let re_ul = Regex::new(r"(?m)^- (.+)$").unwrap();
        processed = re_ul.replace_all(&processed, "- $1").to_string();

        // Ordered lists
        let re_ol = Regex::new(r"(?m)^(\d+)\. (.+)$").unwrap();
        processed = re_ol.replace_all(&processed, "+ $2").to_string();

        processed
    }

    /// Fix newlines for proper Typst formatting
    fn fix_newlines(&self, content: &str) -> String {
        content.replace("\n", "\n  ")
    }

    /// Fix special cases and edge conditions (legacy method, kept for
    /// reference)
    #[deprecated]
    #[allow(dead_code)]
    fn fix_special_cases(&self, content: &str) -> String {
        let mut result = content.to_string();

        // Remove any # prefix from headings in the ontology overview section
        let re = Regex::new(r"^#\s+(.+)$").unwrap();
        result = re.replace_all(&result, "= $1").to_string();

        // Convert URLs in angle brackets to proper Typst links
        let re = Regex::new(r"<(https?://[^>]+)>").unwrap();
        result = re.replace_all(&result, "#link(\"$1\")[\"$1\"]").to_string();

        // Fix quoted strings with language tags (e.g., "text"@en)
        let re = Regex::new(r#""([^"]+)"@[a-z]{2}"#).unwrap();
        result = re.replace_all(&result, "$1").to_string();

        // Remove surrounding quotes from headings and paragraphs
        let re = Regex::new(r#"^(=+\s+)"([^"]+)""#).unwrap();
        result = re.replace_all(&result, "$1$2").to_string();

        // Remove backslashes before text
        let re = Regex::new(r"\\([a-zA-Z])").unwrap();
        result = re.replace_all(&result, "$1").to_string();

        // Remove all backslashes
        result = result.replace("\\", "");

        // Fix double links with extra closing brackets
        let re = Regex::new(
            r#"#link\("([^"]+)"\)\[#link\("([^"]+)"\)\[([^\]]+)\]\]"#,
        )
        .unwrap();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("#link(\"{}\")[\"{}\"]", &caps[2], &caps[3])
            })
            .to_string();

        // Fix any remaining double links
        let re =
            Regex::new(r#"#link\("([^"]+)"\)\[#link\("([^"]+)"\)\[([^\]]+)\]"#)
                .unwrap();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("#link(\"{}\")[\"{}\"]", &caps[2], &caps[3])
            })
            .to_string();

        result
    }
}

// Default implementation for convenience
impl Default for MarkdownToTypstConverter {
    fn default() -> Self { Self::new() }
}
