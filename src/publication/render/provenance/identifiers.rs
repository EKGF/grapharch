//! Validated Git identifiers used by the provenance appendix.
use anyhow::{Result, ensure};

/// Characters shown when a commit is abbreviated.
const SHORT_LENGTH: usize = 12;

/// A full SHA-1 or SHA-256 Git object name in lowercase hexadecimal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CommitSha(String);

impl CommitSha {
    pub(super) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            matches!(value.len(), 40 | 64) &&
                value.bytes().all(|b| {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                }),
            "Git commit identifier must be a full lowercase SHA: {value}"
        );
        Ok(Self(value.into()))
    }

    pub(super) fn as_str(&self) -> &str { &self.0 }

    pub(super) fn short(&self) -> &str { &self.0[..SHORT_LENGTH] }
}

/// The public origin of a source repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RepositoryUrl(url::Url);

impl RepositoryUrl {
    pub(super) fn try_new(value: &str) -> Result<Self> {
        let url = url::Url::parse(value)?;
        ensure!(
            matches!(
                url.scheme(),
                "https" | "http" | "file" | "ssh" | "git"
            ),
            "unsupported provenance repository URL"
        );
        Ok(Self(url))
    }

    pub(super) fn as_str(&self) -> &str { self.0.as_str() }

    /// The hosted commit page, when the forge's commit route is known.
    pub(super) fn commit_url(&self, commit: &CommitSha) -> Option<String> {
        if self.0.scheme() != "https" {
            return None;
        }
        let route = match self.0.host_str()? {
            "github.com" => "commit",
            "gitlab.com" => "-/commit",
            _ => return None,
        };
        let path = self.0.path().trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        (path.split('/').filter(|part| !part.is_empty()).count() >= 2).then(
            || {
                format!(
                    "https://{}{path}/{route}/{}",
                    self.0.host_str().unwrap_or_default(),
                    commit.as_str()
                )
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn commit_sha_rejects_abbreviated_and_uppercase_names() {
        assert!(CommitSha::try_new("0123456789ab").is_err());
        assert!(CommitSha::try_new(&SHA.to_uppercase()).is_err());
        assert_eq!(
            CommitSha::try_new(SHA).unwrap().short(),
            "0123456789ab"
        );
    }

    #[test]
    fn forge_repositories_link_commit_pages() {
        let sha = CommitSha::try_new(SHA).unwrap();
        for (origin, expected) in [
            (
                "https://github.com/EKGF/grapharch",
                format!("https://github.com/EKGF/grapharch/commit/{SHA}"),
            ),
            (
                "https://github.com/EKGF/grapharch.git/",
                format!("https://github.com/EKGF/grapharch/commit/{SHA}"),
            ),
            (
                "https://gitlab.com/group/sub/project",
                format!("https://gitlab.com/group/sub/project/-/commit/{SHA}"),
            ),
        ] {
            assert_eq!(
                RepositoryUrl::try_new(origin).unwrap().commit_url(&sha),
                Some(expected)
            );
        }
        for origin in [
            "ssh://git.example.org/ontology.git",
            "https://git.example.org/ontology",
            "https://github.com/EKGF",
        ] {
            assert_eq!(
                RepositoryUrl::try_new(origin).unwrap().commit_url(&sha),
                None
            );
        }
    }
}
