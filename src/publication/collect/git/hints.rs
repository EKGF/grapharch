//! Bounded publication-hint discovery. Only project and target identities,
//! target links, and the target facts that select the family binding are
//! retained.
use {
    super::{
        MAX_RDF_BYTES,
        blob_url,
        selection::{FamilyTargets, TARGETS, TargetSelection},
    },
    anyhow::{Context, Result, bail, ensure},
    oxrdf::{NamedNode, Quad, Term},
    oxrdfio::{RdfFormat, RdfParser},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::{Component, Path, PathBuf},
    },
};

const PUBLISHES_FAMILY: &str =
    "https://ekgf.org/ontology/artifact-publication#publishesFamily";

/// The hinted project and every target it registers, in IRI order.
pub(crate) struct PublicationHints {
    pub project: String,
    targets:     Vec<NamedNode>,
    renderers:   BTreeMap<String, Vec<Term>>,
    profiles:    BTreeMap<String, Vec<Term>>,
    families:    BTreeSet<String>,
}

/// The kind of hint statement a quad contributes.
enum Hint {
    Project,
    Target,
    Link,
    Renderer,
    Profile,
    Family,
}

impl Hint {
    fn of(quad: &Quad) -> Option<Self> {
        use crate::publication::{RDF_TYPE, term};
        if quad.predicate.as_str() == RDF_TYPE {
            return if quad.object == term("Project").into() {
                Some(Self::Project)
            } else if quad.object == term("Target").into() {
                Some(Self::Target)
            } else {
                None
            };
        }
        if quad.predicate == term("target") {
            Some(Self::Link)
        } else if quad.predicate == term("renderer") {
            Some(Self::Renderer)
        } else if quad.predicate == term("profile") {
            Some(Self::Profile)
        } else if quad.predicate.as_str() == PUBLISHES_FAMILY {
            Some(Self::Family)
        } else {
            None
        }
    }
}

#[derive(Default)]
struct Index {
    projects:         BTreeSet<String>,
    targets:          BTreeMap<String, BTreeSet<String>>,
    declared_targets: BTreeSet<String>,
    renderers:        BTreeMap<String, Vec<Term>>,
    profiles:         BTreeMap<String, Vec<Term>>,
    families:         BTreeSet<String>,
    count:            usize,
    bytes:            usize,
}
impl Index {
    fn observe(&mut self, quad: Quad) -> Result<()> {
        let Some(hint) = Hint::of(&quad) else {
            return Ok(());
        };
        self.count += 1;
        ensure!(
            self.count <= 4096,
            "source exceeds 4096 publication-hint budget"
        );
        let subject: Term = quad.subject.into();
        let Term::NamedNode(subject) = subject else {
            if matches!(hint, Hint::Project | Hint::Target | Hint::Link) {
                bail!(
                    "publication hints require named project and target \
                     identities"
                )
            }
            // A blank node cannot be a registered target or profile.
            return Ok(());
        };
        self.bytes += subject.as_str().len() + quad.object.to_string().len();
        ensure!(
            self.bytes <= 1024 * 1024,
            "publication hints exceed 1 MiB metadata budget"
        );
        let subject = subject.into_string();
        match hint {
            Hint::Project => {
                self.projects.insert(subject);
            },
            Hint::Target => {
                self.declared_targets.insert(subject);
            },
            Hint::Link => {
                let Term::NamedNode(target) = quad.object else {
                    bail!("publication target must be an IRI")
                };
                self.targets
                    .entry(subject)
                    .or_default()
                    .insert(target.into_string());
            },
            Hint::Renderer => {
                let values = self.renderers.entry(subject).or_default();
                if !values.contains(&quad.object) {
                    values.push(quad.object);
                }
            },
            Hint::Profile => {
                let values = self.profiles.entry(subject).or_default();
                if !values.contains(&quad.object) {
                    values.push(quad.object);
                }
            },
            Hint::Family => {
                self.families.insert(subject);
            },
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
        let registered = self.targets.remove(&project).unwrap_or_default();
        ensure!(
            !registered.is_empty(),
            "publication hints project {project} registers no ga:target"
        );
        ensure!(
            registered.len() <= TARGETS,
            "publication hints project {project} registers more than \
             {TARGETS} targets"
        );
        // bounded: at most TARGETS registered targets.
        let mut targets = Vec::with_capacity(registered.len());
        for target in registered {
            ensure!(
                self.declared_targets.contains(&target),
                "publication target {target} must be declared ga:Target"
            );
            targets.push(NamedNode::new(target)?);
        }
        Ok(PublicationHints {
            project,
            targets,
            renderers: self.renderers,
            profiles: self.profiles,
            families: self.families,
        })
    }
}

impl PublicationHints {
    /// Resolve a selection against the registered targets and choose the
    /// target that binds the family: the first selected target with a
    /// family renderer and a profile declaring `artpub:publishesFamily`.
    /// Family bindings belong to that profile, so every other selected target
    /// must use the same profile.
    pub(crate) fn select(
        &self,
        selection: &TargetSelection,
    ) -> Result<FamilyTargets> {
        let selected = match selection {
            TargetSelection::All => self.targets.clone(),
            TargetSelection::Single => {
                ensure!(
                    self.targets.len() == 1,
                    "publication hints project {} registers {} targets ({}); \
                     select the target to publish explicitly",
                    self.project,
                    self.targets.len(),
                    self.listed()
                );
                self.targets.clone()
            },
            TargetSelection::Targets(targets) => {
                for target in targets {
                    ensure!(
                        self.targets.contains(target),
                        "target {target} is not registered by publication \
                         hints project {}",
                        self.project
                    );
                }
                targets.clone()
            },
        };
        if let [_] = selected.as_slice() {
            // The family build validates a sole target against the KG.
            return Ok(FamilyTargets::new(selected, 0));
        }
        let mut primary = None;
        for (index, target) in selected.iter().enumerate() {
            let profile = self.profile(target)?;
            if self.binds_family(target)? &&
                self.families.contains(profile.as_str())
            {
                primary = Some((index, profile));
                break;
            }
        }
        let Some((primary, profile)) = primary else {
            bail!(
                "no selected target of {} publishes a family: a family target \
                 needs a specification, specification-pages or bookshelf \
                 renderer and a profile declaring artpub:publishesFamily ({})",
                self.project,
                selected
                    .iter()
                    .map(NamedNode::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        for target in &selected {
            let used = self.profile(target)?;
            ensure!(
                used == profile,
                "target {target} uses profile {used}, not the family profile \
                 {profile}; a family build publishes only targets of the \
                 family profile, so select the family targets explicitly"
            );
        }
        Ok(FamilyTargets::new(selected, primary))
    }

    fn listed(&self) -> String {
        self.targets
            .iter()
            .map(NamedNode::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn profile(&self, target: &NamedNode) -> Result<NamedNode> {
        match one(&self.profiles, target, "ga:profile")? {
            Term::NamedNode(profile) => Ok(profile.clone()),
            _ => bail!("target {target} ga:profile must be an IRI"),
        }
    }

    fn binds_family(&self, target: &NamedNode) -> Result<bool> {
        let Term::NamedNode(renderer) =
            one(&self.renderers, target, "ga:renderer")?
        else {
            bail!("target {target} ga:renderer must be an IRI")
        };
        Ok(matches!(
            crate::publication::Renderer::from_node(renderer)?.as_str(),
            "specification" | "specification-pages" | "bookshelf"
        ))
    }
}

fn one<'a>(
    values: &'a BTreeMap<String, Vec<Term>>,
    target: &NamedNode,
    property: &str,
) -> Result<&'a Term> {
    let mut found = values.get(target.as_str()).into_iter().flatten();
    let Some(value) = found.next() else {
        bail!("publication hints declare no {property} for target {target}")
    };
    ensure!(
        found.next().is_none(),
        "publication hints declare several {property} values for target \
         {target}"
    );
    Ok(value)
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
