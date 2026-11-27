//! Links from authored pages to repository files that are not pages.
//!
//! A README often links to a file beside it, such as a configuration file or
//! a source file, that discovery does not collect as a page. When a project
//! declares `ga:sourceLinkBase`, the HTTPS location of its repository, such a
//! link renders as `<base>/blob/<commit>/<path>`: the file as it was at the
//! commit the build collected. Collection records that commit and the
//! documentation root's directory in the repository (a `ga:SourceCheckout`),
//! so rendering resolves the link from the collected graph alone.
use {
    crate::publication::{
        collect::source_checkout::SourceCheckout,
        view::RevisionView,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::path::{Component, Path},
    url::Url,
};

/// Project property that enables links to uncollected repository files.
pub(crate) const SETTING: &str = "sourceLinkBase";

/// A full lowercase hexadecimal Git object identifier (SHA-1 or SHA-256).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommitId(String);

impl CommitId {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            matches!(value.len(), 40 | 64) &&
                value
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
            "source commit must be a full lowercase Git object id: {value}"
        );
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

/// A normalized `/`-separated path inside a repository; empty for its root.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RepositoryPath(String);

impl RepositoryPath {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            value.is_empty() ||
                value.split('/').all(|part| {
                    !part.is_empty() &&
                        part != "." &&
                        part != ".." &&
                        !part.contains('\\')
                }),
            "repository path must be relative and normalized: {value}"
        );
        Ok(Self(value.to_owned()))
    }

    /// The path of `path` beneath the repository checkout `root`.
    pub(crate) fn within(root: &Path, path: &Path) -> Result<Self> {
        let relative = path.strip_prefix(root)?;
        let mut parts = Vec::new(); // bounded: components of one path.
        for part in relative.components() {
            let Component::Normal(part) = part else {
                bail!("repository path must not contain traversal")
            };
            parts.push(part.to_str().ok_or_else(|| {
                anyhow::anyhow!("repository path must be UTF-8")
            })?);
        }
        Self::try_new(&parts.join("/"))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }

    /// Resolve the relative link `target` from the file at `self`, refusing
    /// any target that leaves the repository.
    fn resolve(&self, target: &str) -> Result<Self> {
        let mut parts: Vec<&str> = self.0.split('/').collect(); // bounded: components of one path.
        parts.pop();
        parts.retain(|part| !part.is_empty());
        for part in target.split('/') {
            match part {
                "" | "." => {},
                ".." => {
                    ensure!(
                        parts.pop().is_some(),
                        "link {target} is outside the repository; source \
                         links must stay inside it"
                    );
                },
                part => parts.push(part),
            }
        }
        ensure!(
            !parts.is_empty(),
            "link {target} names the repository itself"
        );
        Self::try_new(&parts.join("/")).map_err(|_| {
            anyhow::anyhow!("link {target} is outside the repository")
        })
    }

    fn join(&self, relative: &str) -> String {
        if self.0.is_empty() {
            relative.to_owned()
        } else {
            format!("{}/{relative}", self.0)
        }
    }
}

/// The HTTPS browse location of a project repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceLinkBase(Url);

impl SourceLinkBase {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        let url = Url::parse(value)?;
        ensure!(
            url.scheme() == "https" &&
                url.host_str().is_some() &&
                url.username().is_empty() &&
                url.password().is_none() &&
                url.query().is_none() &&
                url.fragment().is_none(),
            "ga:{SETTING} must be an HTTPS repository IRI without \
             credentials, query or fragment"
        );
        Ok(Self(url))
    }
}

/// Everything needed to link to an uncollected file of the build's commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceLinks {
    base:    SourceLinkBase,
    commit:  CommitId,
    /// The documentation root's directory in the repository.
    content: RepositoryPath,
}

impl SourceLinks {
    pub(crate) fn new(
        base: SourceLinkBase,
        commit: CommitId,
        content: RepositoryPath,
    ) -> Self {
        Self { base, commit, content }
    }

    /// The source links `project` declares, read from the collected graph;
    /// `None` when it does not declare `ga:sourceLinkBase`.
    pub(crate) async fn read(
        view: &RevisionView,
        project: &NamedNode,
    ) -> Result<Option<Self>> {
        let base = match view.optional(project, SETTING).await? {
            None => return Ok(None),
            Some(Term::NamedNode(base)) => {
                SourceLinkBase::try_new(base.as_str())?
            },
            Some(_) => bail!("ga:{SETTING} must be an IRI"),
        };
        let checkout = SourceCheckout::iri(project);
        let Some(Term::Literal(commit)) =
            view.optional(&checkout, "sourceCommit").await?
        else {
            bail!(
                "ga:{SETTING} needs the collected source commit; collect the \
                 documentation from a Git checkout"
            )
        };
        let content = match view.optional(&checkout, "contentDirectory").await?
        {
            None => RepositoryPath::default(),
            Some(Term::Literal(path)) => RepositoryPath::try_new(path.value())?,
            Some(_) => bail!("content directory must be a literal"),
        };
        Ok(Some(Self::new(
            base,
            CommitId::try_new(commit.value())?,
            content,
        )))
    }

    /// The repository URL of the relative `link` written in the page whose
    /// documentation-root-relative source is `page`.
    pub(crate) fn href(&self, page: &str, link: &str) -> Result<String> {
        let (path, fragment) = match link.split_once('#') {
            Some((path, fragment)) => (path, Some(fragment)),
            None => (link, None),
        };
        ensure!(
            !path.is_empty() && !path.starts_with('/') && !path.contains('?'),
            "source link {link} must be a relative file path"
        );
        let path = percent_encoding::percent_decode_str(path).decode_utf8()?;
        let page = RepositoryPath::try_new(&self.content.join(page))?;
        let target = page.resolve(&path)?;
        let mut url = self.base.0.clone();
        {
            let mut segments = url.path_segments_mut().map_err(|_| {
                anyhow::anyhow!("ga:{SETTING} must have a hierarchical path")
            })?;
            segments
                .pop_if_empty()
                .push("blob")
                .push(self.commit.as_str());
            for part in target.as_str().split('/') {
                segments.push(part);
            }
        }
        url.set_fragment(fragment);
        Ok(url.into())
    }
}

#[cfg(test)]
mod tests {
    use super::{CommitId, RepositoryPath, SourceLinkBase, SourceLinks};

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    fn links(base: &str, content: &str) -> SourceLinks {
        SourceLinks::new(
            SourceLinkBase::try_new(base).unwrap(),
            CommitId::try_new(COMMIT).unwrap(),
            RepositoryPath::try_new(content).unwrap(),
        )
    }

    #[test]
    fn links_resolve_from_the_page_inside_the_repository() {
        let root = links("https://github.com/o/r", "");
        assert_eq!(
            root.href("README.md", ".config/environments.yml").unwrap(),
            format!(
                "https://github.com/o/r/blob/{COMMIT}/.config/environments.yml"
            )
        );
        let docs = links("https://github.com/o/r/", "docs");
        assert_eq!(
            docs.href("guide/a.md", "../../src/main%20x.rs#L4").unwrap(),
            format!("https://github.com/o/r/blob/{COMMIT}/src/main%20x.rs#L4")
        );
        assert_eq!(
            docs.href("a.md", "./b/../c.txt").unwrap(),
            format!("https://github.com/o/r/blob/{COMMIT}/docs/c.txt")
        );
    }

    #[test]
    fn links_never_leave_the_repository() {
        let docs = links("https://github.com/o/r", "docs");
        for link in ["../../x", "../..", "/etc/passwd", "a?b", "..\\x", ""] {
            assert!(docs.href("a.md", link).is_err(), "{link}");
        }
    }

    #[test]
    fn identifiers_and_bases_are_validated() {
        assert!(CommitId::try_new("abc").is_err());
        assert!(CommitId::try_new(&COMMIT.to_uppercase()).is_err());
        assert!(SourceLinkBase::try_new("http://github.com/o/r").is_err());
        assert!(SourceLinkBase::try_new("https://u:p@github.com/o/r").is_err());
        assert!(RepositoryPath::try_new("a/../b").is_err());
    }
}
