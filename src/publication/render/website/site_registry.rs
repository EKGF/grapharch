//! Sibling websites of a family of documentation sites.
//!
//! A profile may declare `ga:siteRegistry`, an RDF list of
//! `ga:RegisteredSite` entries, each pairing a repository (`ga:repository`)
//! with the URL of the website published from it (`ga:url`). A Markdown link
//! to such a repository then leads to its website where the website has a
//! page for it: the repository itself leads to the site's home, and a
//! Markdown file beneath the site's content directory (`ga:contentSource`) at
//! its published reference (`ga:gitRef`, when declared) leads to that page.
//! For this website's own repository a page must also exist in this build.
//! Every other repository path, such as source code, stays on the host.
use {
    super::repository_path::RepositoryPath,
    crate::publication::render::model::Plan,
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    url::Url,
};

const MAX_SITES: usize = 64;

/// Where a repository link leads.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Destination {
    /// A page of this website, by its source path below the content root.
    Local(String, Option<String>),
    /// A page of a sibling website.
    Sibling(Url),
}

#[derive(Clone, Debug)]
struct RegisteredSite {
    repository: RepositoryUrl,
    site:       Url,
    content:    String,
    reference:  Option<String>,
}

/// A repository web URL, compared without a trailing slash or `.git`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RepositoryUrl {
    host: String,
    path: String,
}

impl RepositoryUrl {
    pub fn try_new(value: &str) -> Result<Self> {
        let url = Url::parse(value)?;
        ensure!(
            url.scheme() == "https" &&
                url.username().is_empty() &&
                url.password().is_none() &&
                url.query().is_none() &&
                url.fragment().is_none(),
            "registered repository {value} must be an HTTPS URL without \
             credentials, query or fragment"
        );
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("repository {value} lacks a host"))?
            .to_ascii_lowercase();
        let path = url.path().trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        ensure!(
            path.matches('/').count() == 2 && !path.contains("//"),
            "registered repository {value} must name an owner and a repository"
        );
        Ok(Self { host, path: path.to_ascii_lowercase() })
    }

    /// The repository path of `url`, when `url` lies in this repository.
    fn locate<'a>(&self, url: &'a Url) -> Option<RepositoryPath<'a>> {
        if url.scheme() != "https" ||
            url.host_str().map(str::to_ascii_lowercase).as_deref() !=
                Some(self.host.as_str())
        {
            return None;
        }
        let path = url.path();
        let rest = path.get(self.path.len()..)?;
        if !path[..self.path.len()].eq_ignore_ascii_case(&self.path) ||
            !(rest.is_empty() || rest.starts_with('/'))
        {
            return None;
        }
        Some(RepositoryPath::parse(
            rest.trim_start_matches('/'),
        ))
    }
}

#[derive(Default)]
pub(super) struct SiteRegistry {
    // bounded: at most MAX_SITES registered websites.
    sites: Vec<RegisteredSite>,
    own:   Option<RepositoryUrl>,
}

impl SiteRegistry {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let head =
            match plan.source.optional(&plan.profile, "siteRegistry").await? {
                None => return Ok(Self::default()),
                Some(head) => head,
            };
        let members =
            crate::publication::document::term_table::rdf_list::RdfList::new(
                &plan.source,
                head,
            )
            .resources("site registry")
            .await?;
        ensure!(
            members.len() <= MAX_SITES,
            "site registry exceeds {MAX_SITES} websites"
        );
        let mut sites: Vec<RegisteredSite> = Vec::new();
        for member in members {
            let site = RegisteredSite::read(plan, &member).await?;
            ensure!(
                !sites
                    .iter()
                    .any(|known| known.repository == site.repository),
                "site registry lists repository {} twice",
                site.repository.path
            );
            sites.push(site);
        }
        let own =
            match plan.source.optional(&plan.project, "repository").await? {
                Some(Term::NamedNode(repository)) => {
                    RepositoryUrl::try_new(repository.as_str()).ok()
                },
                _ => None,
            };
        Ok(Self { sites, own })
    }

    /// Where an absolute link leads, when it is a page of a registered
    /// website.
    pub fn destination(&self, url: &Url) -> Option<Destination> {
        self.sites.iter().find_map(|site| {
            let path = site.repository.locate(url)?;
            let own = self.own.as_ref() == Some(&site.repository);
            site.destination(&path, url.fragment(), own)
        })
    }
}

impl RegisteredSite {
    async fn read(plan: &Plan, entry: &NamedNode) -> Result<Self> {
        ensure!(
            plan.source.has_type(entry, "RegisteredSite").await?,
            "{entry} is not a declared ga:RegisteredSite"
        );
        let repository = match plan.source.optional(entry, "repository").await?
        {
            Some(Term::NamedNode(repository)) => {
                RepositoryUrl::try_new(repository.as_str())?
            },
            _ => bail!("registered site {entry} requires a ga:repository IRI"),
        };
        let site = Url::parse(&plan.source.text(entry, "url").await?)?;
        ensure!(
            matches!(site.scheme(), "https" | "http") &&
                site.host_str().is_some() &&
                site.query().is_none() &&
                site.fragment().is_none() &&
                site.path().ends_with('/'),
            "registered site {entry} ga:url must be an absolute HTTP(S) \
             directory URL"
        );
        let content = match plan.source.optional(entry, "contentSource").await?
        {
            None => String::new(),
            Some(Term::Literal(content)) => {
                let content = content.value().trim_matches('/');
                ensure!(
                    !content.split('/').any(|part| matches!(part, "." | "..")),
                    "registered site {entry} content directory must stay \
                     inside the repository"
                );
                content.to_owned()
            },
            Some(_) => {
                bail!(
                    "registered site {entry} ga:contentSource must be a \
                     literal"
                )
            },
        };
        let reference = match plan.source.optional(entry, "gitRef").await? {
            None => None,
            Some(Term::Literal(reference)) => {
                Some(reference.value().to_owned())
            },
            Some(_) => {
                bail!("registered site {entry} ga:gitRef must be a literal")
            },
        };
        Ok(Self { repository, site, content, reference })
    }

    fn destination(
        &self,
        path: &RepositoryPath<'_>,
        fragment: Option<&str>,
        own: bool,
    ) -> Option<Destination> {
        let source = match path {
            RepositoryPath::Root => {
                return Some(
                    if own {
                        Destination::Local(String::new(), None)
                    } else {
                        Destination::Sibling(self.site.clone())
                    },
                );
            },
            RepositoryPath::File { reference, path } => {
                if self
                    .reference
                    .as_deref()
                    .is_some_and(|known| known != *reference)
                {
                    return None;
                }
                let source = if self.content.is_empty() {
                    *path
                } else {
                    path.strip_prefix(self.content.as_str())?
                        .strip_prefix('/')?
                };
                source.strip_suffix(".md")?;
                source
            },
            RepositoryPath::Other => return None,
        };
        if own {
            return Some(Destination::Local(
                source.to_owned(),
                fragment.map(str::to_owned),
            ));
        }
        let route =
            crate::publication::collect::page::route(source, None).ok()?;
        let mut url = self.site.join(&route).ok()?;
        url.set_fragment(fragment);
        Some(Destination::Sibling(url))
    }
}
