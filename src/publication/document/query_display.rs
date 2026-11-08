//! Display-only whitespace projection; retained RDF literals stay unchanged.
pub(super) fn format(source: &str) -> String {
    // bounded: the caller caps each resource's combined query literals at 256
    // KiB.
    let mut lines = Vec::new();
    let mut quote: Option<(u8, bool)> = None;
    for line in source.split_inclusive('\n') {
        let protected = quote.is_some();
        scan(line.as_bytes(), &mut quote);
        lines.push((line, protected));
    }
    let blank =
        |(line, protected): &(&str, bool)| !protected && line.trim().is_empty();
    let start = lines
        .iter()
        .position(|line| !blank(line))
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !blank(line))
        .map_or(start, |i| i + 1);
    let lines = &lines[start..end];
    let indent = lines
        .iter()
        .filter(|(line, protected)| !protected && !line.trim().is_empty())
        .map(|(line, _)| {
            line.bytes()
                .take_while(|b| matches!(b, b' ' | b'\t'))
                .count()
        })
        .min()
        .unwrap_or(0);
    let mut display = String::with_capacity(source.len());
    for (line, protected) in lines {
        let remove = if *protected {
            0
        } else {
            line.bytes()
                .take(indent)
                .take_while(|b| matches!(b, b' ' | b'\t'))
                .count()
        };
        display.push_str(&line[remove..]);
    }
    display
}

fn scan(bytes: &[u8], quote: &mut Option<(u8, bool)>) {
    let mut i = 0;
    while i < bytes.len() {
        if let Some((delimiter, long)) = *quote {
            if bytes[i] == b'\\' {
                i += 2;
            } else if bytes[i] == delimiter &&
                (!long || bytes.get(i..i + 3) == Some(&[delimiter; 3]))
            {
                i += if long { 3 } else { 1 };
                *quote = None;
            } else {
                i += 1;
            }
            continue;
        }
        match bytes[i] {
            b'#' => break,
            b'<' => {
                // IRI_REF cannot contain whitespace, quotes, or these
                // delimiters. A comparison operator therefore never consumes
                // later tokens.
                let rest = &bytes[i + 1..];
                let end = rest
                    .iter()
                    .take_while(|b| {
                        !b.is_ascii_whitespace() &&
                            !matches!(
                                b,
                                b'>' | b'<' |
                                    b'"' |
                                    b'\'' |
                                    b'{' |
                                    b'}' |
                                    b'|' |
                                    b'^' |
                                    b'`' |
                                    b'\\'
                            )
                    })
                    .count();
                if rest.get(end) == Some(&b'>') {
                    i += end + 2;
                    continue;
                }
                i += 1;
            },
            delimiter @ (b'"' | b'\'') => {
                let long = bytes.get(i..i + 3) == Some(&[delimiter; 3]);
                *quote = Some((delimiter, long));
                i += if long { 3 } else { 1 };
            },
            _ => i += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format;
    #[test]
    fn removes_turtle_layout_without_changing_query_indentation() {
        let source = "\r\n      PREFIX dct: <http://purl.org/dc/terms/>\r\n      SELECT $this\r\n      WHERE {\r\n        $this ?p ?o .\r\n\r\n      }\r\n    ";
        assert_eq!(
            format(source),
            "PREFIX dct: <http://purl.org/dc/terms/>\r\nSELECT $this\r\nWHERE \
             {\r\n  $this ?p ?o .\r\n\r\n}\r\n"
        );
    }
    #[test]
    fn protects_multiline_string_lexical_whitespace() {
        let source =
            "\n    SELECT * WHERE {\n      BIND(\"\"\"first\n        literal \
             indentation\n\n      last\"\"\" AS ?value)\n      \
             BIND('''single\n   literal''' AS ?other)\n    }\n  ";
        let display = format(source);
        assert!(display.starts_with("SELECT * WHERE {\n  BIND("));
        assert!(display.contains(
            "\"\"\"first\n        literal indentation\n\n      last\"\"\""
        ));
        assert!(display.contains("'''single\n   literal'''"));
    }
    #[test]
    fn quote_markers_in_comments_iris_and_escaped_strings_are_not_long_strings()
    {
        assert_eq!(format("\n  PREFIX x: <https://example.org/#x>\n  # \"\"\" comment\n  ASK { BIND(\"escaped \\\" quote\" AS ?x) }\n"), "PREFIX x: <https://example.org/#x>\n# \"\"\" comment\nASK { BIND(\"escaped \\\" quote\" AS ?x) }\n");
    }
    #[test]
    fn comparison_heavy_query_display_has_bounded_scan_time() {
        let query = "< ".repeat(32768);
        let started = std::time::Instant::now();
        assert_eq!(format(&query), query);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "IRI lookahead repeatedly scanned the remaining query"
        );
    }
}
