//! Project-owned exclusions from automatic documentation source discovery.
//!
//! `ga:excludeSourcePath` names one exact file or directory subtree;
//! `ga:excludeSourcePattern` names files or directories at any depth with a
//! glob, such as `**/AGENTS.md`, and a trailing slash restricts a pattern to
//! directories, such as `**/tests/`. Both are relative to the documentation
//! root and together stay within one 64-entry budget.
use {
    crate::publication::view::RevisionView,
    anyhow::{Result, bail, ensure},
    globset::{GlobBuilder, GlobMatcher},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path, PathBuf},
};

/// Authored exclusions a project may declare.
const MAX_EXCLUSIONS: usize = 64;
/// Longest accepted exclusion pattern, in bytes.
const MAX_PATTERN_BYTES: usize = 2048;

/// Whether a discovered entry is a directory; directory-only patterns match
/// only those, while any pattern excludes everything beneath a matching
/// ancestor directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind {
    File,
    Directory,
}

impl EntryKind {
    pub(crate) fn of(directory: bool) -> Self {
        if directory {
            Self::Directory
        } else {
            Self::File
        }
    }
}

/// A validated, compiled exclusion glob relative to the documentation root.
#[derive(Clone, Debug)]
pub(crate) struct SourcePattern {
    matcher:        GlobMatcher,
    directory_only: bool,
}

impl SourcePattern {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        let directory_only = value.ends_with('/');
        let glob = value.strip_suffix('/').unwrap_or(value);
        ensure!(
            !glob.is_empty() &&
                value.len() <= MAX_PATTERN_BYTES &&
                !glob.starts_with('/') &&
                !glob.contains('\\') &&
                glob.split('/').all(|part| {
                    !part.is_empty() && part != "." && part != ".."
                }),
            "source exclusion pattern must be a relative glob without \
             traversal: {value}"
        );
        let matcher = GlobBuilder::new(glob)
            .literal_separator(true)
            .backslash_escape(false)
            .build()
            .map_err(|error| {
                anyhow::anyhow!(
                    "invalid source exclusion pattern {value}: {error}"
                )
            })?
            .compile_matcher();
        Ok(Self { matcher, directory_only })
    }

    /// Whether `relative`, with `/` separators, or one of its ancestor
    /// directories matches.
    fn matches(&self, relative: &str, kind: EntryKind) -> bool {
        let mut end = 0;
        for (index, part) in relative.split('/').enumerate() {
            end += if index == 0 {
                part.len()
            } else {
                part.len() + 1
            };
            let prefix = &relative[..end];
            let whole = end == relative.len();
            let directory = !whole || kind == EntryKind::Directory;
            if (directory || !self.directory_only) &&
                self.matcher.is_match(prefix)
            {
                return true;
            }
        }
        false
    }
}

/// One authored exclusion.
#[derive(Clone, Debug)]
pub(crate) enum SourceExclusion {
    /// An exact file or directory subtree, joined to the documentation root.
    Path(PathBuf),
    /// Files or directories matching a glob at any depth.
    Pattern(SourcePattern),
}

/// The exclusions of one documentation root.
#[derive(Clone, Debug, Default)]
pub(crate) struct SourceExclusions {
    root:    PathBuf,
    // bounded: at most MAX_EXCLUSIONS authored exclusions.
    entries: Vec<SourceExclusion>,
}

impl SourceExclusions {
    #[cfg(test)]
    pub(crate) fn new(root: &Path, entries: Vec<SourceExclusion>) -> Self {
        Self { root: root.to_owned(), entries }
    }

    /// Read both exclusion forms declared on `project`.
    pub(crate) async fn read(
        root: &Path,
        view: &RevisionView,
        project: &NamedNode,
    ) -> Result<Self> {
        let mut exclusions =
            Self { root: root.to_owned(), entries: Vec::new() };
        let mut rows = view.objects(project, "excludeSourcePath").await?;
        while let Some(row) = rows.next().await? {
            let Term::Literal(value) = &row["value"] else {
                bail!("source exclusion must be a path literal")
            };
            let path = exclusions.path(value.value()).await?;
            exclusions.push(SourceExclusion::Path(path))?;
        }
        let mut rows = view.objects(project, "excludeSourcePattern").await?;
        while let Some(row) = rows.next().await? {
            let Term::Literal(value) = &row["value"] else {
                bail!("source exclusion pattern must be a literal")
            };
            exclusions.push(SourceExclusion::Pattern(SourcePattern::parse(
                value.value(),
            )?))?;
        }
        Ok(exclusions)
    }

    fn push(&mut self, exclusion: SourceExclusion) -> Result<()> {
        ensure!(
            self.entries.len() < MAX_EXCLUSIONS,
            "source exclusions exceed {MAX_EXCLUSIONS} entries"
        );
        self.entries.push(exclusion);
        Ok(())
    }

    async fn path(&self, value: &str) -> Result<PathBuf> {
        let path = Path::new(value);
        ensure!(
            !value.is_empty() &&
                path.components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "source exclusion must be a relative path without traversal"
        );
        let mut ancestor = self.root.clone();
        for part in path.components() {
            ancestor.push(part);
            match tokio::fs::symlink_metadata(&ancestor).await {
                Ok(metadata) => {
                    ensure!(
                        !metadata.is_symlink(),
                        "source exclusion must not traverse a symlink"
                    )
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    break
                },
                Err(error) => return Err(error.into()),
            }
        }
        // Excluded build output need not exist in a clean checkout.
        Ok(self.root.join(path))
    }

    /// Whether discovery skips `path`, an entry of the documentation tree.
    pub(crate) fn excludes(&self, path: &Path, kind: EntryKind) -> bool {
        let relative = path.strip_prefix(&self.root).ok().and_then(|path| {
            let parts = path
                .components()
                .map(|part| {
                    match part {
                        Component::Normal(part) => part.to_str(),
                        _ => None,
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            Some(parts.join("/"))
        });
        self.entries.iter().any(|exclusion| {
            match exclusion {
                SourceExclusion::Path(excluded) => path.starts_with(excluded),
                SourceExclusion::Pattern(pattern) => {
                    relative.as_deref().is_some_and(|relative| {
                        !relative.is_empty() && pattern.matches(relative, kind)
                    })
                },
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{EntryKind, SourceExclusion, SourceExclusions, SourcePattern},
        std::path::Path,
    };

    fn exclusions(patterns: &[&str]) -> SourceExclusions {
        SourceExclusions::new(
            Path::new("/docs"),
            patterns
                .iter()
                .map(|value| {
                    SourceExclusion::Pattern(
                        SourcePattern::parse(value).unwrap(),
                    )
                })
                .collect(),
        )
    }

    fn excluded(set: &SourceExclusions, path: &str, kind: EntryKind) -> bool {
        set.excludes(&Path::new("/docs").join(path), kind)
    }

    #[test]
    fn names_match_at_any_depth() {
        let set = exclusions(&["**/AGENTS.md"]);
        for path in ["AGENTS.md", "a/AGENTS.md", "a/b/c/AGENTS.md"] {
            assert!(excluded(&set, path, EntryKind::File), "{path}");
        }
        for path in ["AGENTS.md.md", "a/MY-AGENTS.md", "a/agents.md"] {
            assert!(!excluded(&set, path, EntryKind::File), "{path}");
        }
    }

    #[test]
    fn directory_patterns_match_directories_and_their_contents() {
        let set = exclusions(&["**/tests/"]);
        assert!(excluded(&set, "tests", EntryKind::Directory));
        assert!(excluded(&set, "a/b/tests", EntryKind::Directory));
        assert!(excluded(&set, "a/tests/case.md", EntryKind::File));
        assert!(excluded(&set, "tests/x/y.md", EntryKind::File));
        assert!(!excluded(&set, "tests", EntryKind::File));
        assert!(!excluded(&set, "a/tests.md", EntryKind::File));
        assert!(!excluded(&set, "a/tests-x/y.md", EntryKind::File));
        assert!(!excluded(
            &set,
            "a/my-tests/y.md",
            EntryKind::File
        ));
    }

    #[test]
    fn single_star_stays_within_one_segment() {
        let set = exclusions(&["drafts/*.md"]);
        assert!(excluded(&set, "drafts/a.md", EntryKind::File));
        assert!(!excluded(
            &set,
            "drafts/old/a.md",
            EntryKind::File
        ));
        assert!(!excluded(
            &set,
            "other/drafts/a.md",
            EntryKind::File
        ));
    }

    #[test]
    fn exact_paths_keep_subtree_semantics() {
        let set = SourceExclusions::new(Path::new("/docs"), vec![
            SourceExclusion::Path(Path::new("/docs/generated").to_owned()),
        ]);
        assert!(excluded(&set, "generated/a.md", EntryKind::File));
        assert!(!excluded(
            &set,
            "generated-not/a.md",
            EntryKind::File
        ));
    }

    #[test]
    fn unsafe_patterns_are_rejected() {
        for value in [
            "", "/", "/a/**", "../a", "a/../b", "./a", "a\\b", "a//b", "[",
        ] {
            assert!(SourcePattern::parse(value).is_err(), "{value}");
        }
    }
}
