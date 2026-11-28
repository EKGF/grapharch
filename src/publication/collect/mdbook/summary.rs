//! `SUMMARY.md` parsed as an outline: prefix/suffix chapters, part titles,
//! nested numbered chapters and draft chapters. Links are read as text only.
use {
    anyhow::{Result, ensure},
    std::path::{Component, Path},
};

/// The largest accepted `SUMMARY.md`.
pub(super) const MAX_SUMMARY_BYTES: u64 = 256 * 1024;
const MAX_ENTRIES: usize = 512;
const MAX_DEPTH: usize = 16;

/// A chapter destination relative to the book's content directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ChapterPath {
    pub path:   String,
    pub anchor: Option<String>,
}

impl ChapterPath {
    fn parse(value: &str) -> Result<Self> {
        let (path, anchor) = value
            .split_once('#')
            .map_or((value, None), |(path, anchor)| {
                (path, Some(anchor))
            });
        let path = path.strip_prefix("./").unwrap_or(path);
        ensure!(
            !path.is_empty() &&
                !path.contains(['?', ':', '\\']) &&
                path.ends_with(".md") &&
                Path::new(path)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "SUMMARY.md destination must be relative to the book source: \
             {value}"
        );
        ensure!(
            anchor.is_none_or(|anchor| !anchor.is_empty()),
            "SUMMARY.md anchor cannot be empty: {value}"
        );
        Ok(Self {
            path:   path.to_owned(),
            anchor: anchor.map(str::to_owned),
        })
    }
}

/// One outline entry, in document order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SummaryEntry {
    pub title:       String,
    /// `None` for a part title or a draft chapter.
    pub destination: Option<ChapterPath>,
    /// Index of the enclosing entry in the outline.
    pub parent:      Option<usize>,
}

/// The parsed outline.
#[derive(Debug, Default)]
pub(super) struct Summary {
    // bounded: at most MAX_ENTRIES.
    pub entries: Vec<SummaryEntry>,
}

impl Summary {
    pub(super) fn parse(body: &str) -> Result<Self> {
        ensure!(
            body.len() as u64 <= MAX_SUMMARY_BYTES,
            "SUMMARY.md exceeds 256 KiB"
        );
        let mut summary = Self::default();
        let mut part = None;
        // bounded: at most MAX_DEPTH open list levels.
        let mut open: Vec<(usize, usize)> = Vec::new();
        let mut started = false;
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || matches!(trimmed, "---" | "***" | "___") {
                continue;
            }
            if let Some(heading) = trimmed.strip_prefix("# ") {
                open.clear();
                if started {
                    part = Some(summary.push(heading.trim(), None, None)?);
                }
                started = true;
                continue;
            }
            started = true;
            let indent = line.len() - line.trim_start().len();
            let item = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "));
            let Some(item) = item else {
                open.clear();
                part = None;
                let (title, destination) = Self::link(trimmed)?;
                summary.push(title, destination, None)?;
                continue;
            };
            while open.last().is_some_and(|(level, _)| *level >= indent) {
                open.pop();
            }
            ensure!(
                open.len() < MAX_DEPTH,
                "SUMMARY.md exceeds depth 16"
            );
            let parent = open.last().map(|(_, index)| *index).or(part);
            let (title, destination) = Self::link(item.trim())?;
            let index = summary.push(title, destination, parent)?;
            open.push((indent, index));
        }
        ensure!(
            !summary.entries.is_empty(),
            "SUMMARY.md lists no chapters"
        );
        Ok(summary)
    }

    /// mdBook publishes `README.md` as the home page, else the first chapter.
    pub(super) async fn home(&self, root: &Path) -> Result<Option<String>> {
        if tokio::fs::try_exists(root.join("README.md")).await? {
            return Ok(None);
        }
        Ok(self
            .entries
            .iter()
            .find_map(|entry| entry.destination.as_ref())
            .map(|destination| destination.path.clone()))
    }

    fn push(
        &mut self,
        title: &str,
        destination: Option<ChapterPath>,
        parent: Option<usize>,
    ) -> Result<usize> {
        ensure!(
            self.entries.len() < MAX_ENTRIES,
            "SUMMARY.md exceeds 512 entries"
        );
        ensure!(
            !title.is_empty(),
            "SUMMARY.md titles cannot be empty"
        );
        self.entries.push(SummaryEntry {
            title: title.to_owned(),
            destination,
            parent,
        });
        Ok(self.entries.len() - 1)
    }

    fn link(text: &str) -> Result<(&str, Option<ChapterPath>)> {
        let unsupported =
            || anyhow::anyhow!("unsupported SUMMARY.md line: {text}");
        let body = text.strip_prefix('[').ok_or_else(unsupported)?;
        let (title, rest) = body.rsplit_once("](").ok_or_else(unsupported)?;
        let destination = rest.strip_suffix(')').ok_or_else(unsupported)?;
        let destination = destination.trim();
        Ok((
            title.trim(),
            (!destination.is_empty())
                .then(|| ChapterPath::parse(destination))
                .transpose()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_keeps_parts_nesting_and_drafts() -> Result<()> {
        let summary = Summary::parse(
            "# Summary\n\n[Intro](./intro.md)\n\n# Part\n\n- [A](a.md)\n  - \
             [B](./b/b.md#x)\n- [Draft]()\n\n---\n\n[End](end.md)\n",
        )?;
        let titles: Vec<_> =
            summary.entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, [
            "Intro", "Part", "A", "B", "Draft", "End"
        ]);
        let parents: Vec<_> =
            summary.entries.iter().map(|e| e.parent).collect();
        assert_eq!(parents, [
            None,
            None,
            Some(1),
            Some(2),
            Some(1),
            None
        ]);
        assert_eq!(
            summary.entries[3].destination,
            Some(ChapterPath {
                path:   "b/b.md".into(),
                anchor: Some("x".into()),
            })
        );
        assert_eq!(summary.entries[4].destination, None);
        Ok(())
    }

    #[test]
    fn rejects_traversal_urls_and_prose() {
        for body in [
            "- [X](../x.md)\n",
            "- [X](https://example.org/x.md)\n",
            "- [X](/etc/x.md)\n",
            "Just prose\n",
        ] {
            assert!(Summary::parse(body).is_err(), "{body}");
        }
    }
}
