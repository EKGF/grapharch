//! The acceptance report: criteria, term differences and page pairs.
use {
    super::{criteria::Finding, criterion::Criterion, pages::PagePair},
    std::{fmt::Write, path::Path},
};

pub struct AcceptanceReport<'a> {
    pub findings: &'a [Finding],
    pub pairs:    &'a [PagePair],
    /// Criteria that are not yet met, each with the work that will meet it.
    pub open:     &'a [(Criterion, &'a str)],
}

impl AcceptanceReport<'_> {
    pub fn markdown(&self) -> String {
        let mut out = String::from(
            "# Commons acceptance: GraphArch edition against OMG \
             formal/26-06-05\n\n| Criterion | Requirement | OMG | GraphArch | \
             Status |\n|---|---|---|---|---|\n",
        );
        for finding in self.findings {
            let status = match (finding.met, self.owner(finding.criterion)) {
                (true, _) => "met".to_owned(),
                (false, Some(owner)) => format!("open: {owner}"),
                (false, None) => "**not met**".to_owned(),
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {status} |",
                finding.criterion,
                finding.criterion.requirement(),
                cell(&finding.omg),
                cell(&finding.ours),
            );
        }
        for finding in self
            .findings
            .iter()
            .filter(|finding| !finding.detail.is_empty())
        {
            let _ = writeln!(out, "\n## {}\n", finding.criterion);
            for line in &finding.detail {
                let _ = writeln!(out, "- {line}");
            }
        }
        out.push_str(
            "\n## Corresponding pages\n\nOMG on the left (blue), GraphArch on \
             the right (green). Layout distance is the mean ink-density \
             difference on a 32 × 32 grid, from 0 (same layout) to 1.\n\n| \
             Page | OMG page | GraphArch page | Layout distance | Image \
             |\n|---|---|---|---|---|\n",
        );
        for pair in self.pairs {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {:.3} | {} |",
                pair.anchor.slug(),
                pair.omg,
                pair.ours,
                pair.distance,
                pair.image.file_name().map_or_else(String::new, |name| {
                    name.to_string_lossy().into_owned()
                }),
            );
        }
        out
    }

    fn owner(&self, criterion: Criterion) -> Option<&str> {
        self.open
            .iter()
            .find(|(open, _)| *open == criterion)
            .map(|(_, owner)| *owner)
    }

    pub fn write(&self, path: &Path) -> anyhow::Result<()> {
        std::fs::write(path, self.markdown())?;
        Ok(())
    }
}

/// A Markdown table cell: one line, pipes escaped, at most 300 characters.
fn cell(text: &str) -> String {
    let text = text.replace('|', "\\|").replace('\n', " ");
    match text.char_indices().nth(300) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text,
    }
}
