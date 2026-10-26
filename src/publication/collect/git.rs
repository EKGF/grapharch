//! Repository discovery and Git provenance. RDF syntax remains a per-file job.
use {
    super::{
        Collector,
        SourceIdentity,
        blank_nodes::BlankNodes,
        source::MAX_RDF_BYTES,
    },
    crate::publication::{dataset::identity, writer::ContributionStream},
    anyhow::{Context, Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    oxrdfio::{RdfFormat, RdfParser},
    std::{
        path::{Path, PathBuf},
        sync::{Arc, atomic::AtomicBool},
    },
};

pub(crate) mod hints;

pub struct DiscoveredRepository {
    pub scan:    RepositoryScan,
    pub project: String,
    pub target:  String,
}

pub struct RepositoryScan {
    pub root:       PathBuf,
    pub commit:     String,
    pub files:      Vec<Arc<dyn Collector>>,
    pub provenance: Arc<dyn Collector>,
}

#[derive(Clone)]
struct Revision {
    project:      String,
    origin:       String,
    commit:       String,
    author:       (String, String, i64),
    committer:    (String, String, i64),
    working_tree: bool,
}
impl Revision {
    fn read(
        project: &str,
        origin: String,
        commit: &gix::Commit<'_>,
        working_tree: bool,
    ) -> Result<Self> {
        let a = commit.author()?;
        let c = commit.committer()?;
        Ok(Self {
            project: project.into(),
            origin,
            commit: commit.id.to_string(),
            author: (
                a.name.to_string(),
                a.email.to_string(),
                a.time()?.seconds,
            ),
            committer: (
                c.name.to_string(),
                c.email.to_string(),
                c.time()?.seconds,
            ),
            working_tree,
        })
    }

    fn entity(&self) -> NamedNode { identity("git-commit", &[&self.commit]) }
}

/// Inspect a local clone without cloning or changing its worktree.
pub async fn local_repository(
    root: &Path,
    project: &str,
) -> Result<RepositoryScan> {
    let root = tokio::fs::canonicalize(root).await?;
    let input = root.clone();
    let project = project.to_owned();
    let revision = tokio::task::spawn_blocking(move || -> Result<Revision> {
        let repo = gix::open(&input).context("open local Git clone")?;
        ensure!(
            repo.workdir() == Some(input.as_path()),
            "local repository must name the checkout root"
        );
        Revision::read(
            &project,
            url::Url::from_directory_path(&input)
                .map_err(|_| anyhow::anyhow!("invalid checkout URL"))?
                .into(),
            &repo.head_commit()?,
            true,
        )
    })
    .await??;
    Ok(RepositoryScan {
        root,
        commit: revision.commit.clone(),
        files: Vec::new(),
        provenance: Arc::new(revision),
    })
}

/// Incrementally fetch one explicit ref into a persistent bare cache. Never
/// checks out files.
pub async fn remote_repository(
    url: &str,
    reference: &str,
    cache: &Path,
    project: &str,
) -> Result<RepositoryScan> {
    ensure!(
        !reference.is_empty(),
        "a Git ref or commit is required"
    );
    let (url, reference, cache, project) = (
        url.to_owned(),
        reference.to_owned(),
        cache.to_owned(),
        project.to_owned(),
    );
    tokio::task::spawn_blocking(move || {
        scan_remote(&url, &reference, &cache, &project)
    })
    .await?
}
/// Fetch once and discover upstream-owned publication hints from that pinned
/// tree.
pub async fn remote_publication(
    url: &str,
    reference: &str,
    cache: &Path,
    hints_path: Option<&Path>,
) -> Result<DiscoveredRepository> {
    ensure!(
        !reference.is_empty(),
        "a Git ref or commit is required"
    );
    let (url, reference, cache, hints_path) = (
        url.to_owned(),
        reference.to_owned(),
        cache.to_owned(),
        hints_path.map(Path::to_owned),
    );
    tokio::task::spawn_blocking(move || {
        let (scan, hints) = scan_remote_selected(
            &url,
            &reference,
            &cache,
            None,
            hints_path.as_deref(),
        )?;
        let hints = hints.ok_or_else(|| {
            anyhow::anyhow!(
                "publication hint discovery did not return identities"
            )
        })?;
        Ok(DiscoveredRepository {
            scan,
            project: hints.project,
            target: hints.target,
        })
    })
    .await?
}

fn scan_remote(
    url: &str,
    reference: &str,
    cache: &Path,
    project: &str,
) -> Result<RepositoryScan> {
    Ok(scan_remote_selected(url, reference, cache, Some(project), None)?.0)
}

fn scan_remote_selected(
    url: &str,
    reference: &str,
    cache: &Path,
    project: Option<&str>,
    hints_path: Option<&Path>,
) -> Result<(RepositoryScan, Option<hints::PublicationHints>)> {
    use fs2::FileExt;
    std::fs::create_dir_all(cache)?;
    let key = identity("git-cache", &[url])
        .as_str()
        .rsplit(':')
        .next()
        .unwrap()
        .to_owned();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(cache.join(format!("{key}.lock")))?;
    lock.lock_exclusive()?;
    let path = cache.join(&key);
    let reused = path.exists();
    let repo = if reused {
        gix::open(&path)?
    } else {
        gix::init_bare(&path)?
    };
    ensure!(repo.is_bare(), "Git cache must be bare");
    let source = if reference == "HEAD" ||
        reference.starts_with("refs/") ||
        gix::hash::ObjectId::from_hex(reference.as_bytes()).is_ok()
    {
        reference.to_owned()
    } else {
        format!("refs/heads/{reference}")
    };
    let spec = format!("+{source}:refs/grapharch/source");
    let remote = repo
        .remote_at(url)?
        .with_refspecs([spec.as_str()], gix::remote::Direction::Fetch)?;
    remote
        .connect(gix::remote::Direction::Fetch)?
        .prepare_fetch(gix::progress::Discard, Default::default())?
        .receive(gix::progress::Discard, &AtomicBool::new(false))
        .context("fetch repository revision into bare cache")?;
    let commit = repo
        .find_reference("refs/grapharch/source")?
        .peel_to_commit()?;
    let mut pending = vec![(String::new(), commit.tree_id()?.detach())];
    let mut entries = Vec::new();
    let mut count = 0;
    while let Some((prefix, id)) = pending.pop() {
        let tree = repo.find_object(id)?.try_into_tree()?;
        for entry in tree.iter() {
            let entry = entry?;
            count += 1;
            ensure!(
                count <= 100_000,
                "repository exceeds 100000-entry budget"
            );
            let name = std::str::from_utf8(entry.filename())?;
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            let relative = format!("{prefix}{name}");
            match entry.mode().kind() {
                gix::objs::tree::EntryKind::Tree => {
                    pending.push((format!("{relative}/"), entry.object_id()))
                },
                gix::objs::tree::EntryKind::Blob |
                gix::objs::tree::EntryKind::BlobExecutable
                    if supported(Path::new(&relative)) =>
                {
                    ensure!(
                        entries.len() < 4096,
                        "repository exceeds 4096 RDF-source budget"
                    );
                    entries.push((relative, entry.object_id()));
                },
                gix::objs::tree::EntryKind::Link
                    if supported(Path::new(&relative)) =>
                {
                    anyhow::bail!(
                        "RDF repository sources cannot be symlinks: {relative}"
                    )
                },
                gix::objs::tree::EntryKind::Commit => {
                    anyhow::bail!(
                        "submodule requires a separately configured \
                         repository: {relative}"
                    )
                },
                _ => {},
            }
        }
    }
    ensure!(
        !entries.is_empty(),
        "repository contains no supported RDF files"
    );
    let public_origin = public_repository_url(url)?.to_string();
    let discovered = if project.is_none() {
        Some(
            hints::discover(
                &repo,
                url,
                &commit.id.to_string(),
                &entries,
                hints_path,
            )
            .with_context(|| {
                format!(
                    "discover publication hints from {public_origin} at \
                     resolved Git commit {}",
                    commit.id
                )
            })?,
        )
    } else {
        None
    };
    let project = project
        .or_else(|| discovered.as_ref().map(|hints| hints.project.as_str()))
        .ok_or_else(|| anyhow::anyhow!("repository project is required"))?;
    let revision = Revision::read(project, public_origin, &commit, false)?;
    let mut files: Vec<Arc<dyn Collector>> = Vec::with_capacity(entries.len());
    for (relative, blob) in entries {
        let base = blob_url(url, &revision.commit, &relative)?;
        let source = identity("git-file", &[url, &relative]).into_string();
        files.push(Arc::new(GitRdfFile {
            project: project.into(),
            processor: identity("family-source", &[project, &relative])
                .into_string(),
            source,
            base,
            relative,
            repository: path.clone(),
            blob,
            revision: revision.clone(),
        }));
    }
    tracing::info!(commit=%revision.commit, files=files.len(), cache_reused=reused, "Git repository pinned");
    Ok((
        RepositoryScan {
            root: path,
            commit: revision.commit.clone(),
            files,
            provenance: Arc::new(revision),
        },
        discovered,
    ))
}
pub(super) fn supported(path: &Path) -> bool {
    path.extension().and_then(|v| v.to_str()).is_some_and(|v| {
        matches!(v, "ttl" | "rdf" | "nt" | "nq" | "trig" | "n3")
    })
}
fn public_repository_url(repository: &str) -> Result<url::Url> {
    let remote = gix::url::parse(repository)?;
    let mut url = match remote.scheme {
        gix::url::Scheme::Ssh => {
            let mut url = url::Url::parse("ssh://localhost")?;
            url.set_host(remote.host.as_deref())?;
            url.set_port(remote.port)
                .map_err(|_| anyhow::anyhow!("invalid SSH port"))?;
            url.set_path(std::str::from_utf8(&remote.path)?);
            url
        },
        gix::url::Scheme::File => {
            let path = if repository.starts_with("file:") {
                url::Url::parse(repository)?.to_file_path().map_err(|_| {
                    anyhow::anyhow!("invalid file repository URL")
                })?
            } else {
                PathBuf::from(std::str::from_utf8(&remote.path)?)
            };
            url::Url::from_file_path(path.canonicalize()?)
                .map_err(|_| anyhow::anyhow!("invalid local repository URL"))?
        },
        _ => url::Url::parse(repository)?,
    };
    if url.scheme() != "file" {
        url.set_username("").map_err(|_| {
            anyhow::anyhow!("invalid repository URL user information")
        })?;
        url.set_password(None).map_err(|_| {
            anyhow::anyhow!("invalid repository URL user information")
        })?;
    }
    Ok(url)
}
fn blob_url(repository: &str, commit: &str, path: &str) -> Result<String> {
    let mut base = public_repository_url(repository)?;
    if base.host_str() == Some("github.com") {
        let path = base.path().to_owned();
        base = url::Url::parse("https://raw.githubusercontent.com")?;
        base.set_path(&path);
    }
    let root = base
        .path()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_owned();
    base.set_path(&root);
    {
        let mut segments = base.path_segments_mut().map_err(|_| {
            anyhow::anyhow!("repository must have a hierarchical URL")
        })?;
        segments.push(commit);
        for part in path.split('/') {
            segments.push(part);
        }
    }
    Ok(base.into())
}
struct GitRdfFile {
    project:    String,
    processor:  String,
    source:     String,
    base:       String,
    relative:   String,
    repository: PathBuf,
    blob:       gix::hash::ObjectId,
    revision:   Revision,
}
#[async_trait]
impl Collector for GitRdfFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: &self.processor,
            source:    &self.source,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let (path, id) = (self.repository.clone(), self.blob);
        let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let repo = gix::open(path)?;
            let header = repo.find_header(id)?;
            ensure!(
                header.size() <= MAX_RDF_BYTES,
                "Git RDF blob exceeds byte budget"
            );
            let mut object = repo.find_object(id)?;
            ensure!(
                object.kind == gix::objs::Kind::Blob,
                "expected Git blob"
            );
            ensure!(
                object.data.len() as u64 <= MAX_RDF_BYTES,
                "Git RDF blob exceeds byte budget"
            );
            Ok(std::mem::take(&mut object.data))
        })
        .await??;
        let format = RdfFormat::from_extension(
            Path::new(&self.relative)
                .extension()
                .unwrap()
                .to_str()
                .unwrap(),
        )
        .context("unsupported RDF blob format")?;
        let revision = format!(
            "git-rdf-v1:{}:{}",
            self.revision.commit, self.blob
        );
        let mut blanks = BlankNodes::new(
            &self.project,
            &self.processor,
            &self.source,
            &revision,
        );
        let mut parser = RdfParser::from_format(format)
            .with_base_iri(&self.base)?
            .for_tokio_async_reader(bytes.as_slice());
        while let Some(quad) = parser.next().await {
            stream.write(blanks.normalize(quad?)).await?;
        }
        stream
            .write(Quad::new(
                NamedNode::new(&self.source)?,
                NamedNode::new("http://www.w3.org/ns/prov#wasDerivedFrom")?,
                self.revision.entity(),
                GraphName::DefaultGraph,
            ))
            .await?;
        stream
            .write(Quad::new(
                NamedNode::new(&self.source)?,
                NamedNode::new("http://www.w3.org/ns/prov#atLocation")?,
                NamedNode::new(&self.base)?,
                GraphName::DefaultGraph,
            ))
            .await?;
        Ok(revision)
    }
}
#[async_trait]
impl Collector for Revision {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: "urn:grapharch:git-provenance",
            source:    &self.origin,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let commit = self.entity();
        let prov = "http://www.w3.org/ns/prov#";
        let repository = identity("git-repository", &[&self.origin]);
        let mut data = vec![
            Quad::new(
                commit.clone(),
                NamedNode::new(crate::publication::RDF_TYPE)?,
                NamedNode::new(format!("{prov}Entity"))?,
                GraphName::DefaultGraph,
            ),
            Quad::new(
                repository.clone(),
                NamedNode::new("http://purl.org/dc/terms/identifier")?,
                Literal::new_simple_literal(&self.origin),
                GraphName::DefaultGraph,
            ),
            Quad::new(
                commit.clone(),
                NamedNode::new("http://purl.org/dc/terms/identifier")?,
                Literal::new_simple_literal(&self.commit),
                GraphName::DefaultGraph,
            ),
            Quad::new(
                commit.clone(),
                NamedNode::new(format!("{prov}wasDerivedFrom"))?,
                repository,
                GraphName::DefaultGraph,
            ),
        ];
        for (role, signature) in
            [("author", &self.author), ("committer", &self.committer)]
        {
            let agent = identity("git-agent", &[&signature.0, &signature.1]);
            let attribution =
                identity("git-attribution", &[&self.commit, role]);
            let role_iri =
                NamedNode::new(format!("urn:grapharch:git-role:{role}"))?;
            for (node, kind) in [
                (&agent, "Person"),
                (&attribution, "Attribution"),
                (&role_iri, "Role"),
            ] {
                data.push(Quad::new(
                    node.clone(),
                    NamedNode::new(crate::publication::RDF_TYPE)?,
                    NamedNode::new(format!("{prov}{kind}"))?,
                    GraphName::DefaultGraph,
                ));
            }
            let assertions: [(NamedNode, String, Term); 6] = [
                (
                    commit.clone(),
                    format!("{prov}qualifiedAttribution"),
                    attribution.clone().into(),
                ),
                (
                    attribution.clone(),
                    format!("{prov}agent"),
                    agent.clone().into(),
                ),
                (
                    attribution,
                    format!("{prov}hadRole"),
                    role_iri.clone().into(),
                ),
                (
                    role_iri,
                    "http://www.w3.org/2000/01/rdf-schema#label".into(),
                    Literal::new_simple_literal(role).into(),
                ),
                (
                    agent.clone(),
                    "http://xmlns.com/foaf/0.1/name".into(),
                    Literal::new_simple_literal(&signature.0).into(),
                ),
                (
                    agent,
                    "https://schema.org/email".into(),
                    Literal::new_simple_literal(&signature.1).into(),
                ),
            ];
            for (s, p, o) in assertions {
                data.push(Quad::new(
                    s,
                    NamedNode::new(p)?,
                    o,
                    GraphName::DefaultGraph,
                ));
            }
            let time = chrono::DateTime::from_timestamp(signature.2, 0)
                .context("invalid Git timestamp")?
                .to_rfc3339();
            let predicate = if role == "author" {
                "http://purl.org/dc/terms/created".into()
            } else {
                format!("{prov}generatedAtTime")
            };
            data.push(Quad::new(
                commit.clone(),
                NamedNode::new(predicate)?,
                Literal::new_typed_literal(
                    time,
                    NamedNode::new(
                        "http://www.w3.org/2001/XMLSchema#dateTime",
                    )?,
                ),
                GraphName::DefaultGraph,
            ));
        }
        if self.working_tree {
            let working = identity("git-working-tree", &[&self.origin]);
            data.push(Quad::new(
                working.clone(),
                NamedNode::new(format!("{prov}wasDerivedFrom"))?,
                commit,
                GraphName::DefaultGraph,
            ));
            data.push(Quad::new(
                working,
                NamedNode::new("http://www.w3.org/2000/01/rdf-schema#label")?,
                Literal::new_simple_literal(
                    "Working tree observed against HEAD; uncommitted files \
                     are collected independently",
                ),
                GraphName::DefaultGraph,
            ));
        }
        for quad in data {
            stream.write(quad).await?;
        }
        Ok(format!(
            "git-provenance-v1:{}:{}",
            self.commit, self.working_tree
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_repository_urls_preserve_spaces_and_native_path_identity()
    -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("a repository with spaces");
        std::fs::create_dir(&path)?;
        let url = url::Url::from_directory_path(&path).unwrap();
        let public = super::public_repository_url(url.as_str())?;
        assert_eq!(
            public.to_file_path().unwrap().canonicalize()?,
            path.canonicalize()?
        );
        Ok(())
    }

    #[test]
    fn git_source_urls_support_scp_syntax_without_exposing_credentials()
    -> anyhow::Result<()> {
        assert_eq!(super::blob_url("git@github.com:EKGF/ekg-ontologies.git", "abc123", "folder/a b.ttl")?, "https://raw.githubusercontent.com/EKGF/ekg-ontologies/abc123/folder/a%20b.ttl");
        let url = super::blob_url(
            "https://secret:token@github.com/EKGF/ekg-ontologies.git",
            "abc123",
            "ontology.ttl",
        )?;
        assert!(!url.contains("secret") && !url.contains("token"));
        Ok(())
    }
}
