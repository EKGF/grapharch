//! A repository on a code forge, normalized from any Git remote spelling.
use {
    anyhow::{Result, bail, ensure},
    url::Url,
};

/// The forge family that hosts a repository; it fixes the URL layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ForgeKind {
    GitHub,
    GitLab,
}

impl ForgeKind {
    /// The forge family of a well-known public host; self-hosted GitLab
    /// instances are recognized by a `gitlab.` host name.
    fn of_host(host: &str) -> Option<Self> {
        match host {
            "github.com" | "www.github.com" => Some(Self::GitHub),
            "gitlab.com" | "www.gitlab.com" => Some(Self::GitLab),
            host if host.starts_with("gitlab.") => Some(Self::GitLab),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::GitLab => "GitLab",
        }
    }

    /// The path segment that precedes browse paths on this forge.
    fn browse_prefix(self) -> &'static [&'static str] {
        match self {
            Self::GitHub => &[],
            Self::GitLab => &["-"],
        }
    }
}

/// A Git commit object id or other ref a browse URL can name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitRef(String);

impl GitRef {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            !value.is_empty() &&
                value.len() <= 255 &&
                !value.starts_with('-') &&
                !value.contains("..") &&
                value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || b"-_./+".contains(&byte)
                }),
            "invalid Git ref {value:?}"
        );
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

/// A repository on a known forge: host, owner namespace and name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ForgeRepository {
    kind:      ForgeKind,
    host:      String,
    /// `owner` on GitHub; a group path such as `group/subgroup` on GitLab.
    namespace: String,
    name:      String,
}

impl ForgeRepository {
    /// Recognize an HTTPS, SSH, `git://` or scp-style (`git@host:o/r.git`)
    /// remote. A remote on an unknown host is not a forge repository.
    pub(crate) fn from_remote(remote: &str) -> Result<Option<Self>> {
        let url = Self::url_of(remote.trim())?;
        let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
            return Ok(None);
        };
        let Some(kind) = ForgeKind::of_host(&host) else {
            return Ok(None);
        };
        let path = url.path().trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let path = path.strip_suffix('/').unwrap_or(path);
        let mut parts: Vec<&str> = path.split('/').collect(); // bounded: one URL path.
        if kind == ForgeKind::GitHub {
            // Browse URLs such as `/o/r/tree/main` name the same repository.
            parts.truncate(2);
        } else if let Some(at) = parts.iter().position(|part| *part == "-") {
            parts.truncate(at);
        }
        ensure!(
            parts.len() >= 2 &&
                parts.iter().all(|part| {
                    !part.is_empty() &&
                        part.len() <= 100 &&
                        *part != "." &&
                        *part != ".." &&
                        part.bytes().all(|c| {
                            c.is_ascii_alphanumeric() || b"-_.".contains(&c)
                        })
                }) &&
                (kind == ForgeKind::GitLab || parts.len() == 2),
            "{} remote {remote} must name owner/repository",
            kind.label()
        );
        let name = parts.pop().unwrap_or_default().to_owned();
        Ok(Some(Self {
            kind,
            host: host.trim_start_matches("www.").to_owned(),
            namespace: parts.join("/"),
            name,
        }))
    }

    fn url_of(remote: &str) -> Result<Url> {
        if let Ok(url) = Url::parse(remote) &&
            url.has_host()
        {
            ensure!(
                matches!(url.scheme(), "https" | "http" | "ssh" | "git"),
                "unsupported remote scheme {}",
                url.scheme()
            );
            return Ok(url);
        }
        // scp-like syntax: [user@]host:path
        let Some((authority, path)) = remote.split_once(':') else {
            bail!("unrecognized Git remote {remote}")
        };
        let host = authority.rsplit('@').next().unwrap_or(authority);
        ensure!(
            !host.is_empty() && !path.starts_with('/') && !host.contains('/'),
            "unrecognized Git remote {remote}"
        );
        Ok(Url::parse(&format!("ssh://{host}/{path}"))?)
    }

    pub(crate) fn kind(&self) -> ForgeKind { self.kind }

    pub(crate) fn host(&self) -> &str { &self.host }

    pub(crate) fn namespace(&self) -> &str { &self.namespace }

    pub(crate) fn name(&self) -> &str { &self.name }

    /// `namespace/name`, the API's project path.
    pub(crate) fn path(&self) -> String {
        format!("{}/{}", self.namespace, self.name)
    }

    /// The repository's HTTPS home page.
    pub(crate) fn web_url(&self) -> Url {
        let mut url = Url::parse(&format!("https://{}/", self.host))
            .expect("validated forge host");
        {
            let mut segments = url.path_segments_mut().expect("https URL");
            segments.clear();
            for part in self.namespace.split('/') {
                segments.push(part);
            }
            segments.push(&self.name);
        }
        url
    }

    fn browse(&self, parts: &[&str]) -> Url {
        let mut url = self.web_url();
        {
            let mut segments = url.path_segments_mut().expect("https URL");
            for part in self.kind.browse_prefix().iter().chain(parts) {
                segments.push(part);
            }
        }
        url
    }

    /// The file `path` (slash-separated, repository-relative) at `at`.
    #[cfg(test)]
    pub(crate) fn blob_url(&self, at: &GitRef, path: &str) -> Url {
        let mut parts = vec!["blob", at.as_str()]; // bounded: one path.
        parts.extend(path.split('/').filter(|part| !part.is_empty()));
        self.browse(&parts)
    }

    pub(crate) fn issues_url(&self) -> Url { self.browse(&["issues"]) }

    /// The release page of one tag.
    pub(crate) fn release_url(&self, tag: &GitRef) -> Url {
        match self.kind {
            ForgeKind::GitHub => {
                self.browse(&["releases", "tag", tag.as_str()])
            },
            ForgeKind::GitLab => self.browse(&["releases", tag.as_str()]),
        }
    }

    /// Discussions exist only on GitHub; whether they are enabled is a forge
    /// API fact.
    pub(crate) fn discussions_url(&self) -> Option<Url> {
        (self.kind == ForgeKind::GitHub).then(|| self.browse(&["discussions"]))
    }
}

#[cfg(test)]
mod tests {
    use super::{ForgeKind, ForgeRepository, GitRef};

    fn parse(remote: &str) -> ForgeRepository {
        ForgeRepository::from_remote(remote).unwrap().unwrap()
    }

    #[test]
    fn every_remote_spelling_normalizes_to_one_repository() {
        for remote in [
            "https://github.com/EKGF/grapharch",
            "https://github.com/EKGF/grapharch.git",
            "https://user:token@github.com/EKGF/grapharch.git",
            "git@github.com:EKGF/grapharch.git",
            "ssh://git@github.com/EKGF/grapharch",
            "git://github.com/EKGF/grapharch.git",
            "https://www.github.com/EKGF/grapharch/tree/main",
        ] {
            let repository = parse(remote);
            assert_eq!(repository.kind(), ForgeKind::GitHub, "{remote}");
            assert_eq!(repository.path(), "EKGF/grapharch", "{remote}");
            assert_eq!(
                repository.web_url().as_str(),
                "https://github.com/EKGF/grapharch"
            );
        }
    }

    #[test]
    fn gitlab_keeps_nested_groups_and_its_browse_layout() {
        let repository = parse("git@gitlab.com:group/sub/project.git");
        assert_eq!(repository.kind(), ForgeKind::GitLab);
        assert_eq!(repository.namespace(), "group/sub");
        let commit = GitRef::try_new("0123abc").unwrap();
        assert_eq!(
            repository.blob_url(&commit, "src/a b.rs").as_str(),
            "https://gitlab.com/group/sub/project/-/blob/0123abc/src/a%20b.rs"
        );
        assert_eq!(
            parse("https://gitlab.com/group/sub/project/-/tree/main").path(),
            "group/sub/project"
        );
        assert!(repository.discussions_url().is_none());
    }

    #[test]
    fn github_urls_follow_its_layout() {
        let repository = parse("https://github.com/o/r");
        let tag = GitRef::try_new("v1.2.0").unwrap();
        assert_eq!(
            repository.blob_url(&tag, "docs/x.md").as_str(),
            "https://github.com/o/r/blob/v1.2.0/docs/x.md"
        );
        assert_eq!(
            repository.release_url(&tag).as_str(),
            "https://github.com/o/r/releases/tag/v1.2.0"
        );
        assert_eq!(
            repository.issues_url().as_str(),
            "https://github.com/o/r/issues"
        );
    }

    #[test]
    fn unknown_hosts_and_invalid_paths_are_not_forge_repositories() {
        assert!(
            ForgeRepository::from_remote("https://example.org/o/r")
                .unwrap()
                .is_none()
        );
        assert!(ForgeRepository::from_remote("https://github.com/o").is_err());
        assert!(
            ForgeRepository::from_remote("https://github.com/o/..").is_err()
        );
        assert!(GitRef::try_new("-x").is_err());
        assert!(GitRef::try_new("a..b").is_err());
    }
}
