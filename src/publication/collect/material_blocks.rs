//! Normalize indentation-based Material authoring syntax at collection time.
use {
    crate::publication::content_blocks::Boundary,
    anyhow::{Result, bail, ensure},
};
pub(super) fn normalize(body: &str) -> Result<String> {
    ensure!(
        body.len() <= 1024 * 1024,
        "Material Markdown exceeds 1 MiB"
    );
    let mut count = 0;
    let normalized = parse(body, 0, &mut count)?;
    Ok(if count == 0 {
        body.to_owned()
    } else {
        normalized
    })
}
fn quoted(value: &str) -> Result<String> {
    let title: String = serde_json::from_str(value.trim()).map_err(|e| {
        anyhow::anyhow!("Material title must be a quoted string: {e}")
    })?;
    ensure!(
        title.len() <= 512,
        "Material title exceeds 512 bytes"
    );
    Ok(title)
}
fn header(line: &str) -> Result<Option<Boundary>> {
    if let Some(rest) = line.strip_prefix("=== ") {
        return Ok(Some(Boundary::Alternative {
            title: quoted(rest)?,
        }));
    }
    let (rest, collapsible, expanded) =
        if let Some(rest) = line.strip_prefix("!!! ") {
            (rest, false, true)
        } else if let Some(rest) = line.strip_prefix("???+ ") {
            (rest, true, true)
        } else if let Some(rest) = line.strip_prefix("??? ") {
            (rest, true, false)
        } else {
            return Ok(None)
        };
    let split = rest.find('"').unwrap_or(rest.len());
    let mut fields = rest[..split].split_whitespace();
    let category = fields.next().ok_or_else(|| {
        anyhow::anyhow!("Material admonition requires a category")
    })?;
    for option in fields {
        ensure!(
            matches!(option, "inline" | "end"),
            "unsupported admonition option: {option}"
        );
    }
    let title = if split < rest.len() {
        quoted(&rest[split..])?
    } else {
        let mut chars = category.chars();
        format!(
            "{}{}",
            chars.next().unwrap().to_ascii_uppercase(),
            chars.as_str()
        )
    };
    let value = Boundary::Admonition {
        category: category.into(),
        title,
        collapsible,
        expanded,
    };
    Boundary::parse(&serde_json::to_string(&value)?)?;
    Ok(Some(value))
}
fn child(lines: &[&str], index: &mut usize) -> String {
    let mut body = String::new();
    while *index < lines.len() {
        let line = lines[*index];
        if line.trim().is_empty() {
            body.push('\n');
            *index += 1;
            continue;
        }
        let Some(content) = line.strip_prefix("    ") else {
            break;
        };
        body.push_str(content);
        body.push('\n');
        *index += 1;
    }
    body
}
fn parse(body: &str, depth: usize, count: &mut usize) -> Result<String> {
    ensure!(depth < 16, "Material nesting exceeds 16");
    // bounded: one source body is capped at 1 MiB; recursive contents shrink.
    let lines: Vec<_> = body.lines().collect();
    let mut result = String::new();
    let mut index = 0;
    let mut fence: Option<(char, usize)> = None;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start_matches(' ');
        let leading = line.len() - trimmed.len();
        let marker = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'));
        if let Some(c) = marker {
            let n = trimmed.chars().take_while(|v| *v == c).count();
            if leading <= 3 && n >= 3 {
                if fence.is_none() {
                    fence = Some((c, n));
                } else if fence.is_some_and(|(old, len)| old == c && n >= len) &&
                    trimmed[n..].trim().is_empty()
                {
                    fence = None;
                }
                result.push_str(line);
                result.push('\n');
                index += 1;
                continue;
            }
        }
        if fence.is_some() {
            result.push_str(line);
            result.push('\n');
            index += 1;
            continue;
        }
        let Some(boundary) = header(line)? else {
            result.push_str(line);
            result.push('\n');
            index += 1;
            continue;
        };
        if matches!(boundary, Boundary::Alternative { .. }) {
            result.push_str(&Boundary::Alternatives.markdown()?);
            loop {
                let Some(tab @ Boundary::Alternative { .. }) =
                    header(lines[index])?
                else {
                    bail!("invalid Material tab group")
                };
                *count += 1;
                ensure!(
                    *count <= 256,
                    "Material container count exceeds 256"
                );
                result.push_str(&tab.markdown()?);
                index += 1;
                let body = child(&lines, &mut index);
                ensure!(
                    !body.trim().is_empty(),
                    "Material tab requires indented content"
                );
                result.push_str(&parse(&body, depth + 1, count)?);
                result.push_str(&Boundary::End.markdown()?);
                if index == lines.len() ||
                    !matches!(
                        header(lines[index])?,
                        Some(Boundary::Alternative { .. })
                    )
                {
                    break;
                }
            }
            result.push_str(&Boundary::End.markdown()?);
        } else {
            *count += 1;
            ensure!(
                *count <= 256,
                "Material container count exceeds 256"
            );
            result.push_str(&boundary.markdown()?);
            index += 1;
            let body = child(&lines, &mut index);
            ensure!(
                !body.trim().is_empty(),
                "Material admonition requires indented content"
            );
            result.push_str(&parse(&body, depth + 1, count)?);
            result.push_str(&Boundary::End.markdown()?);
        }
    }
    ensure!(
        result.len() <= 2 * 1024 * 1024,
        "normalized Material Markdown exceeds byte budget"
    );
    Ok(result)
}
