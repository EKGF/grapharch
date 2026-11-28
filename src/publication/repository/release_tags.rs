//! Semantic-version release tags, of which only those reachable from the
//! checked-out commit describe it.
use {
    super::{forge::GitRef, git_facts::ReleaseTag},
    anyhow::{Result, ensure},
    chrono::DateTime,
    std::collections::HashMap,
};

/// Most tag references examined for the latest release tag.
const MAX_TAGS: usize = 100_000;

/// The highest semantic-version tag of each tagged commit.
pub(super) struct TagCandidates {
    // bounded: MAX_TAGS.
    by_commit: HashMap<gix::ObjectId, (semver::Version, String)>,
    best:      Option<(semver::Version, String, gix::ObjectId)>,
}

impl TagCandidates {
    /// Every tag whose name, `v` prefix allowed, is a semantic version.
    pub(super) fn read(repo: &gix::Repository) -> Result<Self> {
        let mut by_commit: HashMap<_, (semver::Version, String)> =
            HashMap::new();
        let references = repo.references()?;
        for (index, reference) in references.tags()?.enumerate() {
            ensure!(index < MAX_TAGS, "Git tags exceed {MAX_TAGS}");
            let mut reference =
                reference.map_err(|e| anyhow::anyhow!("{e}"))?;
            let name = reference.name().shorten().to_string();
            let Ok(version) = semver::Version::parse(
                name.rsplit('/')
                    .next()
                    .unwrap_or(&name)
                    .trim_start_matches('v'),
            ) else {
                continue;
            };
            let Ok(commit) = reference.peel_to_commit() else {
                continue;
            };
            let id = commit.id;
            if by_commit
                .get(&id)
                .is_none_or(|(current, _)| *current < version)
            {
                by_commit.insert(id, (version, name));
            }
        }
        Ok(Self { by_commit, best: None })
    }

    /// Note a commit of the checked-out history.
    pub(super) fn reach(&mut self, id: gix::ObjectId) {
        let Some((version, name)) = self.by_commit.get(&id) else {
            return
        };
        if self
            .best
            .as_ref()
            .is_none_or(|(current, ..)| current < version)
        {
            self.best = Some((version.clone(), name.clone(), id));
        }
    }

    /// The highest tag reached, which is at or before the checked-out
    /// commit.
    pub(super) fn latest(
        self,
        repo: &gix::Repository,
    ) -> Result<Option<ReleaseTag>> {
        let Some((version, name, id)) = self.best else {
            return Ok(None)
        };
        let date = repo
            .find_commit(id)
            .ok()
            .and_then(|commit| commit.committer().ok()?.time().ok())
            .and_then(|time| DateTime::from_timestamp(time.seconds, 0));
        Ok(Some(ReleaseTag {
            name: GitRef::try_new(&name)?,
            version,
            date,
        }))
    }
}
