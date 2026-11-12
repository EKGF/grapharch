//! Public GitHub profiles of the commit authors of collected GitHub source
//! repositories, counted at the collected commit.
use {
    super::{DCT, PROV, SCHEMA, api, display_text, integer, quad},
    crate::publication::{
        collect::{Collector, SourceIdentity},
        dataset::{Dataset, identity},
        term,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Context, Result, bail, ensure},
    async_trait::async_trait,
    oxrdf::{Literal, NamedNode, Term},
    serde::Deserialize,
    sha2::{Digest, Sha256},
    std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
    },
};

/// Commit history pages of 100 commits examined per repository.
const MAX_COMMIT_PAGES: usize = 10;
const PAGE_SIZE: usize = 100;
/// The most active accounts whose public profiles are fetched and retained.
const MAX_PEOPLE: usize = 32;
const MAX_REPOSITORIES: usize = 8;
const MAX_COMMIT_PAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_USER_BYTES: usize = 64 * 1024;
const GITHUB_API_HOST: &str = "api.github.com";

/// A validated GitHub account name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Login(String);

impl Login {
    pub(super) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            (1..=39).contains(&value.len()) &&
                value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-') &&
                !value.starts_with('-') &&
                !value.ends_with('-'),
            "invalid GitHub login {value:?}"
        );
        Ok(Self(value.to_owned()))
    }

    /// The canonical profile IRI. GitHub logins are case-insensitive, so the
    /// identity is lowercase; the displayed login keeps its spelling.
    pub(super) fn profile(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "https://github.com/{}",
            self.0.to_ascii_lowercase()
        ))
    }
}

/// An `owner/name` repository hosted on github.com.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Repository {
    owner: String,
    name:  String,
}

impl Repository {
    /// Recognize HTTPS and SSH github.com remotes; other hosts are not
    /// GitHub repositories.
    fn from_remote(remote: &str) -> Result<Option<Self>> {
        let url = url::Url::parse(remote)?;
        if !matches!(url.scheme(), "https" | "http" | "ssh" | "git") ||
            !matches!(
                url.host_str(),
                Some("github.com" | "www.github.com")
            )
        {
            return Ok(None);
        }
        let path = url.path().trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let Some((owner, name)) = path.split_once('/') else {
            bail!("GitHub repository {remote} must name owner/repository")
        };
        let valid = |part: &str| {
            !part.is_empty() &&
                part.len() <= 100 &&
                part != "." &&
                part != ".." &&
                part.bytes().all(|c| {
                    c.is_ascii_alphanumeric() || b"-_.".contains(&c)
                })
        };
        ensure!(
            valid(owner) && valid(name),
            "GitHub repository {remote} must name owner/repository"
        );
        Ok(Some(Self {
            owner: owner.into(),
            name:  name.into(),
        }))
    }

    fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "https://github.com/{}/{}",
            self.owner, self.name
        ))
    }
}

pub(super) async fn collectors(
    dataset: &Dataset,
    view: &RevisionView,
    project: &str,
    processor: &NamedNode,
    endpoint: api::Endpoint,
) -> Result<Vec<Arc<dyn Collector>>> {
    let pattern = view.pattern(
        &format!(
            "?repo a <{SCHEMA}SoftwareSourceCode> ; <{SCHEMA}codeRepository> \
             ?url ; <{PROV}hadMember> ?commit . ?commit <{DCT}identifier> ?sha"
        ),
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?url ?sha WHERE {{ {pattern} GRAPH \
             <urn:grapharch:control> {{ ?_attempt0 {} \
             <urn:grapharch:git-provenance> }} }} ORDER BY ?url ?sha LIMIT {}",
            term("collector"),
            4 * MAX_REPOSITORIES + 1
        ))
        .await?;
    let mut repositories = BTreeSet::new(); // bounded: MAX_REPOSITORIES.
    let mut examined = 0;
    while let Some(row) = rows.next().await? {
        examined += 1;
        ensure!(
            examined <= 4 * MAX_REPOSITORIES,
            "GitHub contributor discovery exceeds {} repository records",
            4 * MAX_REPOSITORIES
        );
        let (Term::NamedNode(url), Term::Literal(sha)) =
            (&row["url"], &row["sha"])
        else {
            bail!(
                "Git provenance requires an IRI repository and literal commit"
            )
        };
        ensure!(
            sha.value().len() == 40 &&
                sha.value().bytes().all(|c| c.is_ascii_hexdigit()),
            "Git provenance commit must be a full SHA-1"
        );
        if let Some(repository) = Repository::from_remote(url.as_str())? {
            repositories.insert((repository, sha.value().to_ascii_lowercase()));
        }
        ensure!(
            repositories.len() <= MAX_REPOSITORIES,
            "GitHub contributor collection exceeds {MAX_REPOSITORIES} \
             repository revisions"
        );
    }
    let token = (endpoint.host() == GITHUB_API_HOST)
        .then(|| std::env::var("GITHUB_TOKEN").ok())
        .flatten()
        .filter(|token| !token.trim().is_empty());
    let mut work: Vec<Arc<dyn Collector>> = Vec::new();
    for (repository, commit) in repositories {
        let source = endpoint
            .url(&format!(
                "repos/{}/{}",
                repository.owner, repository.name
            ))?
            .to_string();
        work.push(Arc::new(GitHubContributors {
            project: project.to_owned(),
            processor: processor.as_str().to_owned(),
            endpoint: endpoint.clone(),
            token: token.clone(),
            repository,
            commit,
            source,
        }));
    }
    Ok(work)
}

struct GitHubContributors {
    project:    String,
    processor:  String,
    endpoint:   api::Endpoint,
    /// Sent only to api.github.com, never to another configured host.
    token:      Option<String>,
    repository: Repository,
    commit:     String,
    source:     String,
}

#[derive(Deserialize)]
struct CommitEntry {
    author: Option<Account>,
}
#[derive(Deserialize)]
struct Account {
    login: String,
    #[serde(rename = "type")]
    kind:  String,
}
#[derive(Deserialize)]
struct User {
    login: String,
    name:  Option<String>,
    #[serde(rename = "type")]
    kind:  String,
}

/// Commit counts per author account over the examined history.
struct History {
    counts:       BTreeMap<Login, usize>,
    examined:     usize,
    unattributed: usize,
    complete:     bool,
}

impl History {
    fn record(&mut self, page: Vec<CommitEntry>) -> Result<()> {
        for entry in page {
            self.examined += 1;
            match entry.author {
                Some(account) if account.kind == "User" => {
                    *self
                        .counts
                        .entry(Login::try_new(&account.login)?)
                        .or_default() += 1;
                },
                // Commits without a linked account, and bot or organization
                // accounts, are counted but never become people.
                _ => self.unattributed += 1,
            }
        }
        ensure!(
            self.counts.len() <= MAX_COMMIT_PAGES * PAGE_SIZE,
            "GitHub contributor counts exceed the examined history"
        );
        Ok(())
    }

    /// Most commits first, then case-insensitive login order.
    fn ranked(&self) -> Vec<(Login, usize)> {
        let mut ranked: Vec<_> = self
            .counts
            .iter()
            .map(|(login, count)| (login.clone(), *count))
            .collect(); // bounded: at most one entry per examined commit.
        ranked.sort_by(|left, right| {
            right.1.cmp(&left.1).then_with(|| {
                (left.0.0.to_ascii_lowercase(), &left.0)
                    .cmp(&(right.0.0.to_ascii_lowercase(), &right.0))
            })
        });
        ranked
    }
}

impl GitHubContributors {
    fn request(
        &self,
        client: &reqwest::Client,
        url: url::Url,
    ) -> reqwest::RequestBuilder {
        let request = client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    async fn history(
        &self,
        client: &reqwest::Client,
        hash: &mut Sha256,
    ) -> Result<History> {
        let mut history = History {
            counts:       BTreeMap::new(),
            examined:     0,
            unattributed: 0,
            complete:     false,
        };
        for page in 1..=MAX_COMMIT_PAGES {
            let url = self.endpoint.url(&format!(
                "repos/{}/{}/commits?sha={}&per_page={PAGE_SIZE}&page={page}",
                self.repository.owner, self.repository.name, self.commit
            ))?;
            let body =
                api::get(self.request(client, url), MAX_COMMIT_PAGE_BYTES)
                    .await?;
            hash.update(&body);
            let entries: Vec<CommitEntry> =
                serde_json::from_slice(&body).context("GitHub commit page")?;
            ensure!(
                entries.len() <= PAGE_SIZE,
                "GitHub commit page exceeds the requested size"
            );
            let last = entries.len() < PAGE_SIZE;
            history.record(entries)?;
            if last {
                history.complete = true;
                break;
            }
        }
        Ok(history)
    }

    async fn profile(
        &self,
        client: &reqwest::Client,
        login: &Login,
        hash: &mut Sha256,
    ) -> Result<(url::Url, Option<String>)> {
        let url = self.endpoint.url(&format!("users/{}", login.0))?;
        let body =
            api::get(self.request(client, url.clone()), MAX_USER_BYTES).await?;
        hash.update(&body);
        let user: User =
            serde_json::from_slice(&body).context("GitHub user profile")?;
        ensure!(
            user.login.eq_ignore_ascii_case(&login.0) && user.kind == "User",
            "GitHub profile response does not describe user {}",
            login.0
        );
        let name = user
            .name
            .map(|name| display_text(&name, "GitHub display name"))
            .transpose()?
            .flatten();
        Ok((url, name))
    }
}

#[async_trait]
impl Collector for GitHubContributors {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: &self.processor,
            source:    &self.source,
        })
    }

    async fn produce(&self, output: &mut ContributionStream) -> Result<String> {
        let client = api::Endpoint::client()?;
        let mut hash = Sha256::new();
        let history = self.history(&client, &mut hash).await?;
        let ranked = history.ranked();
        let repository = self.repository.iri();
        for (predicate, object) in [
            ("examinedCommitCount", integer(history.examined)),
            (
                "unattributedCommitCount",
                integer(history.unattributed),
            ),
            ("contributorCount", integer(ranked.len())),
            (
                "commitHistoryComplete",
                Literal::from(history.complete),
            ),
            (
                "examinedCommit",
                Literal::new_simple_literal(&self.commit),
            ),
        ] {
            output
                .write(quad(
                    &repository,
                    term(predicate).as_str(),
                    object,
                ))
                .await?;
        }
        for (login, count) in ranked.into_iter().take(MAX_PEOPLE) {
            let (url, name) = self.profile(&client, &login, &mut hash).await?;
            let person = login.profile();
            let title = name.unwrap_or_else(|| login.0.clone());
            let contribution = identity("github-contribution", &[
                repository.as_str(),
                person.as_str(),
            ]);
            for assertion in [
                quad(
                    &person,
                    crate::publication::RDF_TYPE,
                    NamedNode::new_unchecked(format!("{SCHEMA}Person")),
                ),
                quad(
                    &person,
                    &format!("{DCT}title"),
                    Literal::new_simple_literal(&title),
                ),
                quad(
                    &person,
                    &format!("{SCHEMA}name"),
                    Literal::new_simple_literal(&title),
                ),
                quad(
                    &person,
                    "http://xmlns.com/foaf/0.1/accountName",
                    Literal::new_simple_literal(&login.0),
                ),
                quad(
                    &person,
                    term("url").as_str(),
                    Literal::new_simple_literal(format!(
                        "https://github.com/{}",
                        login.0
                    )),
                ),
                quad(
                    &person,
                    &format!("{PROV}wasDerivedFrom"),
                    NamedNode::new(url.as_str())?,
                ),
                quad(
                    &repository,
                    &format!("{SCHEMA}contributor"),
                    person.clone(),
                ),
                quad(
                    &contribution,
                    crate::publication::RDF_TYPE,
                    term("RepositoryContribution"),
                ),
                quad(
                    &contribution,
                    term("contributionTo").as_str(),
                    repository.clone(),
                ),
                quad(
                    &contribution,
                    &format!("{PROV}agent"),
                    person.clone(),
                ),
                quad(
                    &contribution,
                    term("commitCount").as_str(),
                    integer(count),
                ),
            ] {
                output.write(assertion).await?;
            }
        }
        Ok(format!(
            "github-contributors-v1:{}:{}",
            self.commit,
            hex::encode(hash.finalize())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Account, CommitEntry, History, Login, Repository};

    fn entry(login: Option<&str>, kind: &str) -> CommitEntry {
        CommitEntry {
            author: login.map(|login| {
                Account { login: login.into(), kind: kind.into() }
            }),
        }
    }

    #[test]
    fn contributors_rank_by_commits_then_login_and_exclude_bots() {
        let mut history = History {
            counts:       Default::default(),
            examined:     0,
            unattributed: 0,
            complete:     true,
        };
        history
            .record(vec![
                entry(Some("zed"), "User"),
                entry(Some("Alice"), "User"),
                entry(Some("bob"), "User"),
                entry(Some("zed"), "User"),
                entry(Some("dependabot[bot]"), "Bot"),
                entry(None, "User"),
            ])
            .unwrap();
        let ranked: Vec<_> = history
            .ranked()
            .into_iter()
            .map(|(login, count)| (login.0, count))
            .collect();
        assert_eq!(ranked, [
            ("zed".to_owned(), 2),
            ("Alice".to_owned(), 1),
            ("bob".to_owned(), 1)
        ]);
        assert_eq!((history.examined, history.unattributed), (6, 2));
        assert_eq!(
            Login::try_new("Alice").unwrap().profile().as_str(),
            "https://github.com/alice"
        );
        assert!(Login::try_new("bad/login").is_err());
    }

    #[test]
    fn only_github_remotes_name_repositories() {
        assert_eq!(
            Repository::from_remote("ssh://github.com/EKGF/dprod.git")
                .unwrap()
                .unwrap()
                .iri()
                .as_str(),
            "https://github.com/EKGF/dprod"
        );
        assert!(
            Repository::from_remote("https://gitlab.com/a/b")
                .unwrap()
                .is_none()
        );
        assert!(Repository::from_remote("https://github.com/a").is_err());
    }
}
