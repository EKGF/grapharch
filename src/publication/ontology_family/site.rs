//! Website targets of a family build. Their pages come from the project's
//! declared `ga:contentSource`, read from the same source as the family: a
//! directory or checkout as is, a pinned repository at the scanned commit.
use {
    crate::publication::{
        collect::{Collector, git::RepositoryScan},
        view::RevisionView,
        website::SitePages,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// Where a family's sources live, for reading its website pages.
pub(super) enum SiteSource {
    /// A directory tree or working-tree checkout.
    Directory(PathBuf),
    /// A pinned repository scan; pages are read from its commit.
    Repository(RepositoryScan),
}

/// Page collectors for the selected website targets. A materialized pinned
/// tree must outlive collection.
pub(super) struct FamilySite {
    pub(super) websites:   Vec<NamedNode>,
    pub(super) collectors: Vec<Arc<dyn Collector>>,
    _tree:                 Option<tempfile::TempDir>,
}

impl SiteSource {
    /// Take a pinned scan's file collectors and keep the scan for its pages.
    pub(super) fn pinned(
        mut scan: RepositoryScan,
        site: &mut Option<Self>,
    ) -> Vec<Arc<dyn Collector>> {
        let mut inputs = std::mem::take(&mut scan.files);
        inputs.push(scan.provenance.clone());
        *site = Some(Self::Repository(scan));
        inputs
    }
}

impl FamilySite {
    /// Prepare the pages of every selected website target.
    pub(super) async fn prepare(
        source: Option<&SiteSource>,
        view: &RevisionView,
        project: &str,
        targets: &[NamedNode],
    ) -> Result<Self> {
        // bounded: a selection names at most 16 targets.
        let mut websites = Vec::new();
        for target in targets {
            if view.renderer(target).await? ==
                crate::publication::Renderer::Website
            {
                websites.push(target.clone());
            }
        }
        if websites.is_empty() {
            return Ok(Self { websites, collectors: Vec::new(), _tree: None });
        }
        let project_id = NamedNode::new(project)?;
        let Some(content) = view.optional(&project_id, "contentSource").await?
        else {
            bail!(
                "website target {} needs ga:contentSource on {project} naming \
                 the directory of its pages",
                websites[0]
            )
        };
        let Term::Literal(content) = content else {
            bail!("ga:contentSource must be a relative directory path literal")
        };
        let Some(source) = source else {
            bail!(
                "website target {} of a family build needs a directory or Git \
                 source for its pages",
                websites[0]
            )
        };
        let (collectors, tree) = match source {
            SiteSource::Directory(root) => {
                let pages = SitePages {
                    source:          root,
                    repository:      None,
                    mkdocs_metadata: false,
                    skip:            None,
                };
                (pages.collectors(view, project).await?, None)
            },
            SiteSource::Repository(scan) => {
                let tree = tempfile::tempdir()?;
                let mut selected = vec![PathBuf::from(content.value())];
                let mut roots =
                    view.objects(&project_id, "includeRoot").await?;
                while let Some(row) = roots.next().await? {
                    ensure!(
                        selected.len() < 9,
                        "includeRoot exceeds eight-directory budget"
                    );
                    let Term::Literal(root) = &row["value"] else {
                        bail!(
                            "includeRoot must be a relative directory path \
                             literal"
                        )
                    };
                    selected.push(PathBuf::from(root.value()));
                }
                scan.materialize_documentation(&selected, tree.path())
                    .await?;
                let root = tokio::fs::canonicalize(tree.path()).await?;
                let collectors =
                    Self::pinned(&root, scan, view, project).await?;
                (collectors, Some(tree))
            },
        };
        ensure!(
            !collectors.is_empty(),
            "ga:contentSource {} of {project} contains no website pages",
            content.value()
        );
        Ok(Self { websites, collectors, _tree: tree })
    }

    async fn pinned(
        root: &Path,
        scan: &RepositoryScan,
        view: &RevisionView,
        project: &str,
    ) -> Result<Vec<Arc<dyn Collector>>> {
        SitePages {
            source:          root,
            repository:      Some(scan),
            mkdocs_metadata: false,
            skip:            None,
        }
        .collectors(view, project)
        .await
    }
}
