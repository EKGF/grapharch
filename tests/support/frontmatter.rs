use anyhow::{Context, Result};

pub fn parse(source: &str) -> Result<(&str, &str)> {
    let mut lines = source.split_inclusive('\n');
    let opening = lines.next().context("missing YAML frontmatter")?;
    anyhow::ensure!(
        opening.trim_end_matches(['\r', '\n']) == "---",
        "missing YAML frontmatter"
    );
    let start = opening.len();
    let mut offset = start;
    for line in lines {
        let end = offset;
        offset += line.len();
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Ok((&source[start..end], &source[offset..]));
        }
    }
    anyhow::bail!("unterminated YAML frontmatter")
}

#[test]
fn yaml_frontmatter_framing_accepts_lf_and_windows_crlf() -> Result<()> {
    for newline in ["\n", "\r\n"] {
        let source =
            ["---", "title: A guide", "---", "# Guide", ""].join(newline);
        let (metadata, body) = parse(&source)?;
        #[derive(serde::Deserialize)]
        struct Page {
            title: String,
        }
        assert_eq!(
            serde_yaml::from_str::<Page>(metadata)?.title,
            "A guide"
        );
        assert!(body.starts_with("# Guide"));
    }
    Ok(())
}

#[test]
fn yaml_frontmatter_requires_complete_delimiter_lines() {
    for source in [
        "title: No block",
        "---\ntitle: Unclosed",
        "---\ntitle: '---'\ntext",
    ] {
        assert!(parse(source).is_err());
    }
}
