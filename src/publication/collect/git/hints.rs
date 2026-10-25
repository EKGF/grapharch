//! Bounded publication-hint discovery. Only identities and target links are
//! retained.
use {
    super::{MAX_RDF_BYTES, blob_url},
    anyhow::{Context, Result, ensure},
    oxrdf::{Quad, Term},
    oxrdfio::{RdfFormat, RdfParser},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::{Component, Path, PathBuf},
    },
};

pub(crate) struct PublicationHints {
    pub project: String,
    pub target:  String,
}
#[derive(Default)]
struct Index {
    projects:         BTreeSet<String>,
    targets:          BTreeMap<String, BTreeSet<String>>,
    declared_targets: BTreeSet<String>,
    count:            usize,
    bytes:            usize,
}
impl Index {
    fn observe(&mut self, quad: Quad) -> Result<()> {
        let is_project = quad.predicate.as_str() ==
            crate::publication::RDF_TYPE &&
            quad.object == crate::publication::term("Project").into();
        let is_target = quad.predicate.as_str() == crate::publication::RDF_TYPE &&
            quad.object == crate::publication::term("Target").into();
        let link = quad.predicate == crate::publication::term("target");
        if !is_project && !is_target && !link {
            return Ok(());
        }
        self.count += 1;
        ensure!(
            self.count <= 4096,
            "source exceeds 4096 publication-hint budget"
        );
        let subject: Term = quad.subject.into();
        let Term::NamedNode(subject) = subject else {
            anyhow::bail!(
                "publication hints require named project and target identities"
            )
        };
        self.bytes += subject.as_str().len() + quad.object.to_string().len();
        ensure!(
            self.bytes <= 1024 * 1024,
            "publication hints exceed 1 MiB metadata budget"
        );
        if is_project {
            self.projects.insert(subject.into_string());
        } else if is_target {
            self.declared_targets.insert(subject.into_string());
        } else {
            let Term::NamedNode(target) = quad.object else {
                anyhow::bail!("publication target must be an IRI")
            };
            self.targets
                .entry(subject.into_string())
                .or_default()
                .insert(target.into_string());
        }
        Ok(())
    }

    fn finish(mut self) -> Result<PublicationHints> {
        ensure!(
            self.projects.len() == 1,
            "publication hints require exactly one ga:Project; found {}",
            self.projects.len()
        );
        let project = self.projects.into_iter().next().unwrap();
        let targets = self.targets.remove(&project).unwrap_or_default();
        ensure!(
            targets.len() == 1,
            "publication hints require exactly one project target; found {}",
            targets.len()
        );
        let target = targets.into_iter().next().unwrap();
        ensure!(
            self.declared_targets.contains(&target),
            "publication target must be declared ga:Target"
        );
        Ok(PublicationHints { project, target })
    }
}
fn parse(
    index: &mut Index,
    bytes: &[u8],
    path: &Path,
    base: &str,
) -> Result<()> {
    let format = RdfFormat::from_extension(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or(""),
    )
    .context("unsupported publication hints RDF format")?;
    for quad in RdfParser::from_format(format)
        .with_base_iri(base)?
        .for_reader(bytes)
    {
        index.observe(quad?).with_context(|| {
            format!("read publication hints in {}", path.display())
        })?;
    }
    Ok(())
}
fn relative(path: &Path) -> Result<()> {
    ensure!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "publication hints path must be relative to the source"
    );
    Ok(())
}
pub(super) fn discover(
    repo: &gix::Repository,
    url: &str,
    commit: &str,
    entries: &[(String, gix::hash::ObjectId)],
    selected: Option<&Path>,
) -> Result<PublicationHints> {
    if let Some(selected) = selected {
        relative(selected)?;
        ensure!(
            entries.iter().any(|(path, _)| Path::new(path) == selected),
            "publication hints file is not a supported RDF blob in the \
             resolved repository"
        );
    }
    let mut index = Index::default();
    for (path, id) in entries {
        if selected.is_some_and(|selected| Path::new(path) != selected) {
            continue;
        }
        ensure!(
            repo.find_header(*id)?.size() <= MAX_RDF_BYTES,
            "publication hints RDF blob exceeds byte budget: {path}"
        );
        let object = repo.find_object(*id)?;
        ensure!(
            object.data.len() as u64 <= MAX_RDF_BYTES,
            "publication hints RDF blob exceeds byte budget: {path}"
        );
        parse(
            &mut index,
            &object.data,
            Path::new(path),
            &blob_url(url, commit, path)?,
        )?;
    }
    index.finish()
}
pub(crate) fn discover_files(
    root: &Path,
    paths: &[PathBuf],
    selected: Option<&Path>,
) -> Result<PublicationHints> {
    let selected = if let Some(path) = selected {
        relative(path)?;
        let path = root.join(path).canonicalize()?;
        ensure!(
            path.starts_with(root) && paths.contains(&path),
            "publication hints file is not a supported RDF source beneath the \
             source root"
        );
        Some(path)
    } else {
        None
    };
    let mut index = Index::default();
    for path in paths {
        if selected.as_ref().is_some_and(|selected| selected != path) {
            continue;
        }
        ensure!(
            std::fs::metadata(path)?.len() <= MAX_RDF_BYTES,
            "publication hints RDF file exceeds byte budget: {}",
            path.display()
        );
        let bytes = std::fs::read(path)?;
        ensure!(
            bytes.len() as u64 <= MAX_RDF_BYTES,
            "publication hints RDF file exceeds byte budget: {}",
            path.display()
        );
        let base = url::Url::from_file_path(path).map_err(|_| {
            anyhow::anyhow!("invalid publication hints file URI")
        })?;
        parse(&mut index, &bytes, path, base.as_str())?;
    }
    index.finish()
}
