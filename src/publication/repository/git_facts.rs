//! Repository facts read from a Git work tree with gix; never changes it.
use {
    super::forge::{ForgeRepository, GitRef},
    anyhow::{Context, Result, ensure},
    chrono::{DateTime, Utc},
    std::path::{Path, PathBuf},
};

/// Most tag references examined for the latest release tag.
const MAX_TAGS: usize = 100_000;
/// Most first-parent commits walked to find the first commit.
const MAX_HISTORY: usize = 10_000_000;

/// A full lowercase Git object id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommitSha(String);

impl CommitSha {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            matches!(value.len(), 40 | 64) &&
                value
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "invalid commit id {value}"
        );
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

/// The highest semantic-version tag in the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReleaseTag {
    pub(crate) name:    GitRef,
    pub(crate) version: semver::Version,
    pub(crate) date:    Option<DateTime<Utc>>,
}

/// What the work tree says about its repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitFacts {
    pub(crate) root:           PathBuf,
    pub(crate) head:           CommitSha,
    pub(crate) default_branch: Option<GitRef>,
    pub(crate) latest_tag:     Option<ReleaseTag>,
    pub(crate) dirty:          bool,
    pub(crate) origin:         Option<ForgeRepository>,
    pub(crate) first_commit:   DateTime<Utc>,
    pub(crate) last_commit:    DateTime<Utc>,
    /// Whether history ends at a shallow boundary, so the first commit date
    /// is only the earliest observed one.
    pub(crate) shallow:        bool,
}

impl GitFacts {
    /// Facts of the work tree enclosing `source`, or `None` outside Git.
    pub(crate) async fn discover(source: &Path) -> Result<Option<Self>> {
        let source = source.to_owned();
        tokio::task::spawn_blocking(move || {
            let Some(root) =
                source.ancestors().find(|dir| dir.join(".git").exists())
            else {
                return Ok(None);
            };
            let repo = gix::open(root).context("open Git repository")?;
            let Some(root) = repo.workdir().map(Path::to_owned) else {
                return Ok(None);
            };
            Self::read(&repo, root).map(Some)
        })
        .await?
    }

    fn read(repo: &gix::Repository, root: PathBuf) -> Result<Self> {
        let head = repo.head_commit().context("read HEAD commit")?;
        let last_commit = time(head.committer()?.time()?.seconds)?;
        let shallow = repo.shallow_commits()?;
        let mut first = last_commit;
        let mut current = Some(head.id);
        let mut walked = 0;
        let mut truncated = false;
        while let Some(id) = current.take() {
            walked += 1;
            ensure!(
                walked <= MAX_HISTORY,
                "Git history exceeds {MAX_HISTORY}"
            );
            let commit = repo.find_commit(id)?;
            first = time(commit.committer()?.time()?.seconds)?.min(first);
            if shallow
                .as_ref()
                .is_some_and(|commits| commits.contains(&id))
            {
                truncated = true;
                break;
            }
            current = commit.parent_ids().next().map(|id| id.detach());
        }
        let origin = match repo.try_find_remote("origin") {
            Some(remote) => {
                remote?.url(gix::remote::Direction::Fetch).and_then(|url| {
                    ForgeRepository::from_remote(&url.to_bstring().to_string())
                        .ok()
                        .flatten()
                })
            },
            None => None,
        };
        Ok(Self {
            root,
            head: CommitSha::try_new(&head.id.to_string())?,
            default_branch: default_branch(repo)?,
            latest_tag: latest_tag(repo)?,
            dirty: repo.is_dirty().context("read work tree status")?,
            origin,
            first_commit: first,
            last_commit,
            shallow: truncated,
        })
    }
}

fn time(seconds: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp(seconds, 0).context("invalid Git timestamp")
}

/// `origin/HEAD`'s target, else the checked-out branch.
fn default_branch(repo: &gix::Repository) -> Result<Option<GitRef>> {
    if let Some(reference) =
        repo.try_find_reference("refs/remotes/origin/HEAD")? &&
        let gix::refs::TargetRef::Symbolic(target) = reference.target()
    {
        let name = target.as_bstr().to_string();
        if let Some(branch) = name.strip_prefix("refs/remotes/origin/") {
            return Ok(GitRef::try_new(branch).ok());
        }
    }
    Ok(repo
        .head_name()?
        .and_then(|name| GitRef::try_new(&name.shorten().to_string()).ok()))
}

/// The tag with the highest semantic version, `v` prefix allowed.
fn latest_tag(repo: &gix::Repository) -> Result<Option<ReleaseTag>> {
    let mut best: Option<(semver::Version, String, gix::ObjectId)> = None;
    let references = repo.references()?;
    for (index, reference) in references.tags()?.enumerate() {
        ensure!(index < MAX_TAGS, "Git tags exceed {MAX_TAGS}");
        let mut reference = reference.map_err(|e| anyhow::anyhow!("{e}"))?;
        let name = reference.name().shorten().to_string();
        let Ok(version) = semver::Version::parse(
            name.rsplit('/')
                .next()
                .unwrap_or(&name)
                .trim_start_matches('v'),
        ) else {
            continue;
        };
        if best
            .as_ref()
            .is_some_and(|(current, ..)| *current >= version)
        {
            continue;
        }
        let id = reference.peel_to_id()?.detach();
        best = Some((version, name, id));
    }
    let Some((version, name, id)) = best else {
        return Ok(None)
    };
    let date = repo
        .find_object(id)?
        .peel_to_commit()
        .ok()
        .and_then(|commit| commit.committer().ok()?.time().ok())
        .and_then(|time| DateTime::from_timestamp(time.seconds, 0));
    Ok(Some(ReleaseTag {
        name: GitRef::try_new(&name)?,
        version,
        date,
    }))
}
