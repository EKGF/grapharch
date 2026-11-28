//! The newest commit that changed each collected page source: the date and
//! author behind "Last updated" (git-revision-date-localized parity).
//!
//! One bounded walk over first-parent history from HEAD compares each
//! still-pending path's blob with the parent's; memory is a map bounded by
//! the number of collected sources. Sources outside a Git work tree get no
//! record.
use {
    crate::publication::{
        RDF_TYPE,
        collect::{Collector, SourceIdentity},
        dataset::{Dataset, identity},
        term,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Context, Result, ensure},
    async_trait::async_trait,
    chrono::{DateTime, Utc},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// Most commits examined for one repository.
const MAX_COMMITS: usize = 1_000_000;
/// Most page sources dated per build.
const MAX_SOURCES: usize =
    crate::publication::limits::PUBLICATION_ENTRIES as usize;
const PROCESSOR: &str = "urn:grapharch:page-revisions";

/// The newest change of one source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PageRevision {
    pub(crate) date:   DateTime<Utc>,
    pub(crate) author: String,
}

/// Newest change per repository-relative path at HEAD of `root`.
pub(crate) fn newest_changes(
    root: &Path,
    paths: &[String],
) -> Result<BTreeMap<String, PageRevision>> {
    let repo = gix::open(root).context("open Git repository")?;
    let mut pending: Vec<&String> = paths.iter().collect(); // bounded: paths.
    let mut found = BTreeMap::new(); // bounded: paths.
    let mut current = Some(repo.head_commit()?.id);
    let mut walked = 0;
    while let Some(id) = current.take() {
        if pending.is_empty() {
            break;
        }
        walked += 1;
        ensure!(
            walked <= MAX_COMMITS,
            "Git history exceeds {MAX_COMMITS} commits"
        );
        let commit = repo.find_commit(id)?;
        let tree = commit.tree()?;
        let parent = commit.parent_ids().next().map(|id| id.detach());
        let parent_tree = match parent {
            Some(parent) => {
                match repo.find_commit(parent) {
                    Ok(parent) => Some(parent.tree()?),
                    // A shallow boundary: the oldest observed commit owns the
                    // rest.
                    Err(_) => None,
                }
            },
            None => None,
        };
        let signature = commit.author()?;
        let revision = PageRevision {
            date:   DateTime::from_timestamp(signature.time()?.seconds, 0)
                .context("invalid Git timestamp")?,
            author: signature.name.to_string(),
        };
        let mut index = 0;
        while index < pending.len() {
            let path = pending[index];
            let here = tree
                .lookup_entry_by_path(path)?
                .map(|entry| entry.object_id());
            let before = match &parent_tree {
                Some(parent) => {
                    parent.lookup_entry_by_path(path)?.map(|e| e.object_id())
                },
                None => None,
            };
            if here.is_some() && here != before {
                found.insert(path.clone(), revision.clone());
                pending.swap_remove(index);
            } else {
                index += 1;
            }
        }
        current = parent_tree.and(parent);
    }
    Ok(found)
}

/// Dates for the collected page sources of `project`, as one collector.
pub(crate) async fn collectors(
    dataset: &Dataset,
    project: &str,
) -> Result<Vec<Arc<dyn Collector>>> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_collected_for(project).await?,
    );
    let pattern = view.pattern(
        &format!(
            "?fragment {} ?path ; {} ?contribution . ?contribution {} ?file",
            term("sourcePath"),
            term("contribution"),
            term("source")
        ),
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?path ?file WHERE {{ {pattern} }}"
        ))
        .await?;
    let mut by_repository: BTreeMap<PathBuf, Vec<(String, String)>> =
        BTreeMap::new(); // bounded: MAX_SOURCES.
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        let (Term::Literal(path), Term::Literal(file)) =
            (&row["path"], &row["file"])
        else {
            continue;
        };
        let file = Path::new(file.value());
        if !file.is_absolute() {
            continue;
        }
        let Some(root) = file.ancestors().find(|dir| dir.join(".git").exists())
        else {
            continue;
        };
        let Ok(relative) = file.strip_prefix(root) else {
            continue
        };
        count += 1;
        ensure!(
            count <= MAX_SOURCES,
            "page revisions exceed {MAX_SOURCES} sources"
        );
        by_repository.entry(root.to_owned()).or_default().push((
            path.value().to_owned(),
            relative.to_string_lossy().replace('\\', "/"),
        ));
    }
    if by_repository.is_empty() {
        return Ok(Vec::new());
    }
    let project = NamedNode::new(project)?;
    let revisions = tokio::task::spawn_blocking(
        move || -> Result<Vec<(String, PageRevision)>> {
            let mut dated = Vec::new(); // bounded: MAX_SOURCES.
            for (root, sources) in by_repository {
                let paths: Vec<String> =
                    sources.iter().map(|(_, repo)| repo.clone()).collect();
                let found = newest_changes(&root, &paths)?;
                for (page, repo) in sources {
                    if let Some(revision) = found.get(&repo) {
                        dated.push((page, revision.clone()));
                    }
                }
            }
            dated.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(dated)
        },
    )
    .await??;
    Ok(vec![Arc::new(PageRevisions {
        project,
        revisions,
    })])
}

/// The page revisions of one build.
struct PageRevisions {
    project:   NamedNode,
    revisions: Vec<(String, PageRevision)>, // bounded: MAX_SOURCES.
}

impl PageRevisions {
    /// The record of the page whose source path is `path`.
    fn node(project: &NamedNode, path: &str) -> NamedNode {
        identity("page-revision", &[project.as_str(), path])
    }
}

#[async_trait]
impl Collector for PageRevisions {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.project.as_str(),
            processor: PROCESSOR,
            source:    self.project.as_str(),
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let mut digest = String::new();
        for (path, revision) in &self.revisions {
            let node = Self::node(&self.project, path);
            for (predicate, object) in [
                (
                    NamedNode::new_unchecked(RDF_TYPE),
                    Term::from(term("PageRevision")),
                ),
                (
                    term("sourcePath"),
                    Literal::new_simple_literal(path).into(),
                ),
                (
                    NamedNode::new_unchecked(
                        "http://purl.org/dc/terms/modified",
                    ),
                    Literal::new_typed_literal(
                        revision.date.to_rfc3339(),
                        NamedNode::new_unchecked(
                            "http://www.w3.org/2001/XMLSchema#dateTime",
                        ),
                    )
                    .into(),
                ),
                (
                    NamedNode::new_unchecked("https://schema.org/author"),
                    Literal::new_simple_literal(&revision.author).into(),
                ),
            ] {
                stream
                    .write(Quad::new(
                        node.clone(),
                        predicate,
                        object,
                        GraphName::DefaultGraph,
                    ))
                    .await?;
            }
            digest.push_str(&format!("{path}={};", revision.date.timestamp()));
        }
        Ok(format!(
            "page-revisions-v1:{}",
            hex::encode(<sha2::Sha256 as sha2::Digest>::digest(
                digest.as_bytes()
            ))
        ))
    }
}
