//! A streaming walk over one source root with every exclusion layer applied.
use {
    super::{budget::SourceBudget, defaults::DefaultExclusions},
    crate::publication::{
        view::RevisionView,
        website::exclusions::{EntryKind, SourceExclusions, SourcePattern},
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path, PathBuf},
};

/// Project property whose globs bring default-excluded paths back.
pub(crate) const INCLUDE_SETTING: &str = "includeSourcePattern";
/// Most inclusion patterns a project may declare.
const MAX_INCLUSIONS: usize = 64;
/// Most navigation entries honoured as explicit inclusions.
const MAX_NAVIGATED: usize = 4096;
/// An ignore file in Git syntax that applies only to GraphArch discovery.
pub(crate) const GRAPHARCH_IGNORE: &str = ".grapharchignore";

/// Which files a discovery selects.
pub(crate) type Accept = fn(&Path) -> bool;

/// Discovery of the sources beneath one root.
///
/// Layers, in order: hidden and `_`-prefixed names; the repository's ignore
/// files (`.gitignore`, `.git/info/exclude`, the global Git excludes file,
/// `.ignore` and `.grapharchignore`), applied whether or not a `.git`
/// directory exists; the project's authored exclusions; the
/// [`DefaultExclusions`] unless an inclusion pattern matches. Symbolic links
/// are never followed.
pub(crate) struct SourceWalk {
    root:       PathBuf,
    accept:     Accept,
    skip:       Option<PathBuf>,
    authored:   SourceExclusions,
    inclusions: Vec<SourcePattern>, // bounded: MAX_INCLUSIONS.
    /// Paths an explicit navigation entry names; they win over defaults.
    navigated:  std::collections::BTreeSet<String>, // bounded: MAX_NAVIGATED.
    budget:     SourceBudget,
}

impl SourceWalk {
    pub(crate) fn new(root: &Path, accept: Accept) -> Self {
        Self {
            root: root.to_owned(),
            accept,
            skip: None,
            authored: SourceExclusions::default(),
            inclusions: Vec::new(),
            navigated: std::collections::BTreeSet::new(),
            budget: SourceBudget::DEFAULT,
        }
    }

    /// Never descend into `path`, such as the build's own output directory.
    pub(crate) fn skipping(mut self, path: PathBuf) -> Self {
        self.skip = Some(path);
        self
    }

    pub(crate) fn with_budget(mut self, budget: SourceBudget) -> Self {
        self.budget = budget;
        self
    }

    pub(crate) fn with_exclusions(
        mut self,
        authored: SourceExclusions,
    ) -> Self {
        self.authored = authored;
        self
    }

    pub(crate) fn with_inclusions(
        mut self,
        inclusions: Vec<SourcePattern>,
    ) -> Result<Self> {
        ensure!(
            inclusions.len() <= MAX_INCLUSIONS,
            "ga:{INCLUDE_SETTING} exceeds {MAX_INCLUSIONS} patterns"
        );
        self.inclusions = inclusions;
        Ok(self)
    }

    /// The project's exclusions, inclusions and budget.
    pub(crate) async fn configured(
        self,
        view: &RevisionView,
        project: &NamedNode,
    ) -> Result<Self> {
        let authored =
            SourceExclusions::read(&self.root, view, project).await?;
        let budget = SourceBudget::read(view, project).await?;
        let mut inclusions = Vec::new(); // bounded: MAX_INCLUSIONS.
        let mut rows = view.objects(project, INCLUDE_SETTING).await?;
        while let Some(row) = rows.next().await? {
            let Term::Literal(value) = &row["value"] else {
                bail!("ga:{INCLUDE_SETTING} must be a glob literal")
            };
            ensure!(
                inclusions.len() < MAX_INCLUSIONS,
                "ga:{INCLUDE_SETTING} exceeds {MAX_INCLUSIONS} patterns"
            );
            inclusions.push(SourcePattern::parse(value.value())?);
        }
        let navigated = navigated(view).await?;
        self.with_exclusions(authored)
            .with_budget(budget)
            .with_inclusions(inclusions)
            .map(|walk| walk.with_navigated(navigated))
    }

    /// Paths, relative to the root, that explicit navigation (MkDocs `nav`,
    /// `SUMMARY.md`, `.pages`) names: a default exclusion never drops them.
    pub(crate) fn with_navigated(
        mut self,
        paths: std::collections::BTreeSet<String>,
    ) -> Self {
        self.navigated = paths;
        self
    }

    /// The selected files, sorted.
    pub(crate) async fn files(self) -> Result<Vec<PathBuf>> {
        tokio::task::spawn_blocking(move || self.walk()).await?
    }

    fn walk(self) -> Result<Vec<PathBuf>> {
        let defaults = DefaultExclusions::new()?;
        let mut builder = ignore::WalkBuilder::new(&self.root);
        builder
            .hidden(true)
            .parents(false)
            .ignore(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .require_git(false)
            .follow_links(false)
            .add_custom_ignore_filename(GRAPHARCH_IGNORE)
            .sort_by_file_path(Ord::cmp);
        let root = self.root.clone();
        let skip = self.skip.clone();
        let authored = self.authored.clone();
        let prune_defaults =
            self.inclusions.is_empty() && self.navigated.is_empty();
        let directory_defaults = defaults.clone();
        builder.filter_entry(move |entry| {
            let path = entry.path();
            if path == root {
                return true;
            }
            let directory = entry.file_type().is_some_and(|kind| kind.is_dir());
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('_') ||
                skip.as_deref() == Some(path) ||
                authored.excludes(path, EntryKind::of(directory))
            {
                return false;
            }
            !(directory &&
                prune_defaults &&
                relative(&root, path).is_some_and(|relative| {
                    directory_defaults.excludes(&relative, EntryKind::Directory)
                }))
        });
        let label = self.root.display().to_string();
        let mut files = Vec::new(); // bounded: the source budget.
        for entry in builder.build() {
            let entry = entry?;
            let Some(kind) = entry.file_type() else {
                continue
            };
            if !kind.is_file() || !(self.accept)(entry.path()) {
                continue;
            }
            let Some(relative) = relative(&self.root, entry.path()) else {
                bail!(
                    "source names must be UTF-8: {}",
                    entry.path().display()
                )
            };
            if defaults.excludes(&relative, EntryKind::File) &&
                !self.navigated.contains(&relative) &&
                !self.inclusions.iter().any(|pattern| {
                    pattern.matches(&relative, EntryKind::File)
                })
            {
                continue;
            }
            self.budget.admit(files.len(), &label)?;
            files.push(entry.into_path());
        }
        Ok(files)
    }
}

/// `/`-separated path of `path` beneath `root`.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let parts = path
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|part| {
            match part {
                Component::Normal(part) => part.to_str(),
                _ => None,
            }
        })
        .collect::<Option<Vec<_>>>()?; // bounded: components of one path.
    Some(parts.join("/"))
}

/// Source paths of the navigation entries already in `view`.
async fn navigated(
    view: &RevisionView,
) -> Result<std::collections::BTreeSet<String>> {
    let pattern = view.pattern(
        &format!(
            "?entry a {} ; {} ?path",
            crate::publication::term("NavigationEntry"),
            crate::publication::term("sourcePath")
        ),
        0,
    );
    let mut rows = view
        .dataset()
        .select(format!(
            "SELECT DISTINCT ?path WHERE {{ {pattern} }}"
        ))
        .await?;
    let mut paths = std::collections::BTreeSet::new(); // bounded: MAX_NAVIGATED.
    while let Some(row) = rows.next().await? {
        let Term::Literal(path) = &row["path"] else {
            bail!("navigation source path must be a literal")
        };
        ensure!(
            paths.len() < MAX_NAVIGATED,
            "navigation names more than {MAX_NAVIGATED} source paths"
        );
        paths.insert(path.value().trim_start_matches("./").to_owned());
    }
    Ok(paths)
}
