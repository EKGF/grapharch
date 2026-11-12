//! A registry-selected family is collected once and published from one pinned
//! KG.
use {
    super::{
        assembly::Assembler,
        collect::{
            Collector,
            RdfFile,
            collect_all,
            git::{FamilyTargets, TargetSelection},
        },
        dataset::{Dataset, identity},
        document::{
            AnnotatedTermDocumentor,
            Documentor,
            namespace_bindings::NamespaceBindings,
        },
        plan::{ResolvedPlan, TargetPlanner},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    futures::{StreamExt, TryStreamExt},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    std::{
        collections::{BTreeMap, HashSet},
        path::{Path, PathBuf},
        sync::Arc,
    },
};
pub(crate) mod arrangement;
mod declarations;
mod modules;
mod reference;

const GOVERNANCE: &str = "https://ekgf.org/ontology/artifact-governance#";
const PUBLICATION: &str = "https://ekgf.org/ontology/artifact-publication#";
pub struct CollectedFamily {
    pub dataset:        Dataset,
    /// Plan of the target whose profile receives the family bindings.
    pub plan:           ResolvedPlan,
    /// Plans of every selected target, including `plan`, in publication
    /// order. Each publishes into its own declared destination.
    pub plans:          Vec<ResolvedPlan>,
    pub ontology_count: usize,
    pub term_count:     usize,
}
struct Member {
    ontology:  NamedNode,
    scheme:    bool,
    prefix:    String,
    title:     String,
    namespace: String,
    view:      RevisionView,
    snapshot:  NamedNode,
    terms:     Vec<NamedNode>,
    missing:   usize,
}

/// Source discovery is separate from the specialized per-document collectors.
pub enum FamilySource<'a> {
    Directory(&'a Path),
    PreparedRepository(super::collect::git::RepositoryScan),
    LocalRepository(&'a Path),
    RemoteRepository {
        url:       &'a str,
        reference: &'a str,
        cache:     &'a Path,
    },
    Sparql(&'a [SparqlFamilyCollector]),
    RdfUrls(&'a [RdfUrlFamilyCollector]),
}
/// One explicitly selected native RDF document published over HTTP(S).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RdfUrlFamilyCollector {
    pub url:        String,
    pub media_type: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SparqlFamilyCollector {
    pub collector: String,
    pub source:    super::collect::SparqlSourceConfig,
}

pub async fn collect(
    source: &Path,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedFamily> {
    collect_from(
        Dataset::memory()?,
        FamilySource::Directory(source),
        definitions,
        project,
        target,
        concurrency,
    )
    .await
}
pub async fn collect_repository(
    source: &Path,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedFamily> {
    collect_from(
        Dataset::memory()?,
        FamilySource::LocalRepository(source),
        definitions,
        project,
        target,
        concurrency,
    )
    .await
}
#[allow(clippy::too_many_arguments)]
pub async fn collect_remote(
    url: &str,
    reference: &str,
    cache: &Path,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedFamily> {
    collect_from(
        Dataset::memory()?,
        FamilySource::RemoteRepository { url, reference, cache },
        definitions,
        project,
        target,
        concurrency,
    )
    .await
}
/// Run the family stages against an explicitly selected shared KG.
pub async fn collect_from(
    dataset: Dataset,
    source: FamilySource<'_>,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedFamily> {
    collect_from_optional(
        dataset,
        source,
        Some(definitions),
        project,
        &FamilyTargets::single(NamedNode::new(target)?),
        concurrency,
    )
    .await
}

/// Use upstream configuration already present in a prepared repository's RDF
/// sources.
pub async fn collect_prepared(
    dataset: Dataset,
    repository: super::collect::git::DiscoveredRepository,
    concurrency: usize,
) -> Result<CollectedFamily> {
    let commit = repository.scan.commit.clone();
    let origin = repository.scan.provenance.identity()?.source.to_owned();
    collect_from_optional(
        dataset,
        FamilySource::PreparedRepository(repository.scan),
        None,
        &repository.project,
        &repository.targets,
        concurrency,
    )
    .await
    .map_err(|error| {
        error.context(format!(
            "build publication from {origin} at resolved Git commit {commit}"
        ))
    })
}

/// Discover upstream RDF publication identities while retaining the caller's KG
/// backend.
pub async fn collect_discovered_from(
    dataset: Dataset,
    source: FamilySource<'_>,
    hints_path: Option<&Path>,
    selection: &TargetSelection,
    concurrency: usize,
) -> Result<CollectedFamily> {
    match source {
        FamilySource::RemoteRepository { url, reference, cache } => {
            let repository = super::collect::git::remote_publication(
                url, reference, cache, hints_path, selection,
            )
            .await?;
            collect_prepared(dataset, repository, concurrency).await
        },
        FamilySource::Directory(path) | FamilySource::LocalRepository(path) => {
            let root = tokio::fs::canonicalize(path).await?;
            let paths = discover(&root).await?;
            let hint_path = hints_path.map(Path::to_owned);
            let read_root = root.clone();
            let hints = tokio::task::spawn_blocking(move || {
                super::collect::git::hints::discover_files(
                    &read_root,
                    &paths,
                    hint_path.as_deref(),
                )
            })
            .await??;
            let targets = hints.select(selection)?;
            collect_from_optional(
                dataset,
                source,
                None,
                &hints.project,
                &targets,
                concurrency,
            )
            .await
        },
        FamilySource::PreparedRepository(_) => {
            anyhow::bail!(
                "prepared repository requires discovered project and target \
                 identities"
            )
        },
        FamilySource::Sparql(_) | FamilySource::RdfUrls(_) => {
            anyhow::bail!(
                "SPARQL sources require explicit publication definitions"
            )
        },
    }
}

async fn collect_from_optional(
    dataset: Dataset,
    source: FamilySource<'_>,
    definitions: Option<&Path>,
    project: &str,
    targets: &FamilyTargets,
    concurrency: usize,
) -> Result<CollectedFamily> {
    let target = targets.primary().as_str();
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 16, 4 * 1024 * 1024)?;
    let mut managed =
        ManagedFamily::begin(&dataset, &writer, project, target).await?;
    let result = async {
        let definitions = match definitions {
            Some(path) => Some(tokio::fs::canonicalize(path).await?),
            None => None,
        };
        let inputs = match source {
            FamilySource::Directory(path) => {
                file_collectors(
                    &tokio::fs::canonicalize(path).await?,
                    definitions.as_deref(),
                    project,
                )
                .await?
            },
            FamilySource::LocalRepository(path) => {
                let scan = super::collect::git::local_repository(path, project)
                    .await?;
                let mut inputs = file_collectors(
                    &scan.root,
                    definitions.as_deref(),
                    project,
                )
                .await?;
                inputs.push(scan.provenance);
                inputs
            },
            FamilySource::RemoteRepository { url, reference, cache } => {
                let scan = super::collect::git::remote_repository(
                    url, reference, cache, project,
                )
                .await?;
                let mut inputs = scan.files;
                inputs.push(scan.provenance);
                inputs
            },
            FamilySource::PreparedRepository(scan) => {
                let mut inputs = scan.files;
                inputs.push(scan.provenance);
                inputs
            },
            FamilySource::RdfUrls(configs) => {
                ensure!(
                    !configs.is_empty() && configs.len() <= 4096,
                    "family RDF URL sources require between 1 and 4096 \
                     documents"
                );
                // bounded: explicit source inventory, capped at 4096 documents.
                let mut inputs: Vec<Arc<dyn Collector>> = Vec::new();
                let mut seen = HashSet::new();
                for config in configs {
                    ensure!(
                        seen.insert(config.url.as_str()),
                        "duplicate family RDF URL source"
                    );
                    let collector =
                        identity("family-rdf-url", &[project, &config.url]);
                    inputs.push(Arc::new(super::collect::RdfUrl::new(
                        project,
                        collector.as_str(),
                        &config.url,
                        &config.media_type,
                    )?));
                }
                inputs
            },
            FamilySource::Sparql(configs) => {
                ensure!(
                    !configs.is_empty() && configs.len() <= 4096,
                    "family SPARQL sources require between 1 and 4096 \
                     collectors"
                );
                let mut inputs: Vec<Arc<dyn Collector>> = Vec::new();
                for config in configs {
                    inputs.push(Arc::new(
                        super::collect::SparqlEndpointSource::new(
                            project,
                            &config.collector,
                            config.source.clone(),
                        )?,
                    ));
                }
                inputs
            },
        };
        build(
            inputs,
            definitions.as_deref(),
            &dataset,
            &writer,
            project,
            targets,
            concurrency,
            &mut managed,
        )
        .await
    }
    .await;
    if result.is_err() {
        managed.abort().await?;
    }
    drop(managed);
    drop(writer);
    worker.await??;
    let (plans, ontology_count, term_count) = result?;
    let plan = plans[targets
        .selected()
        .iter()
        .position(|selected| selected == targets.primary())
        .expect("the primary target is selected")]
    .clone();
    Ok(CollectedFamily { dataset, plan, plans, ontology_count, term_count })
}

async fn file_collectors(
    root: &Path,
    definitions: Option<&Path>,
    project: &str,
) -> Result<Vec<Arc<dyn Collector>>> {
    let mut inputs: Vec<Arc<dyn Collector>> = Vec::new();
    for path in discover(root).await? {
        if definitions.is_some_and(|definitions| path == definitions) {
            continue;
        }
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("family path must be UTF-8"))?;
        let processor = identity("family-source", &[project, relative]);
        inputs.push(Arc::new(RdfFile::new(
            project,
            processor.as_str(),
            path,
        )?));
    }
    Ok(inputs)
}
async fn discover(root: &Path) -> Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    let mut count = 0;
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            count += 1;
            ensure!(
                count <= 100_000,
                "family discovery exceeds entry budget"
            );
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("family paths must be UTF-8"))?;
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            let kind = entry.file_type().await?;
            ensure!(
                !kind.is_symlink(),
                "family discovery rejects symlinks"
            );
            if kind.is_dir() {
                pending.push(entry.path());
            } else if entry.path().extension().is_some_and(|extension| {
                matches!(
                    extension.to_str(),
                    Some("ttl" | "rdf" | "nt" | "nq" | "trig" | "n3")
                )
            }) {
                files.push(entry.path());
                ensure!(
                    files.len() <= 4096,
                    "family exceeds 4096 RDF-source budget"
                );
            }
        }
    }
    files.sort();
    ensure!(
        !files.is_empty(),
        "family source contains no supported RDF files"
    );
    Ok(files)
}
#[allow(clippy::too_many_arguments)]
async fn build(
    inputs: Vec<Arc<dyn Collector>>,
    definitions: Option<&Path>,
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    targets: &FamilyTargets,
    concurrency: usize,
    managed: &mut ManagedFamily,
) -> Result<(Vec<ResolvedPlan>, usize, usize)> {
    let target = targets.primary().as_str();
    let started = std::time::Instant::now();
    let mut work: Vec<Arc<dyn Collector>> = Vec::new();
    if let Some(definitions) = definitions {
        work.push(Arc::new(RdfFile::new(
            project,
            "urn:grapharch:family-definitions",
            definitions.to_owned(),
        )?));
    }
    let mut sources = Vec::new();
    for collector in inputs {
        sources.push(NamedNode::new(collector.identity()?.processor)?);
        work.push(collector);
    }
    for input in &work {
        let id = input.identity()?;
        managed.add(id.processor, id.source, false)?;
    }
    managed.persist_inventory(writer, true).await?;
    collect_all(work, writer, concurrency).await?;
    let people = super::collect::people::collectors(dataset, project).await?;
    for input in &people {
        let id = input.identity()?;
        managed.add(id.processor, id.source, false)?;
    }
    managed.persist_inventory(writer, true).await?;
    collect_all(people, writer, concurrency).await?;
    managed.retire(writer, false).await?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        "family sources collected"
    );
    let snapshot = dataset.snapshot_collected_for(project).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    let target = NamedNode::new(target)?;
    ensure!(
        view.renderer(&target).await?.publishes_family(),
        "family target {target} requires a specification, \
         specification-pages, bookshelf or typst renderer"
    );
    let profile = view.node(&target, "profile").await?;
    let scope = view.node(&target, "scope").await?;
    let family = view
        .optional_by(
            &profile,
            &NamedNode::new(format!("{PUBLICATION}publishesFamily"))?,
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "family profile must explicitly declare artpub:publishesFamily"
            )
        })?;
    let Term::NamedNode(family) = family else {
        anyhow::bail!("family identity must be an IRI")
    };
    let mut family_types = view
        .objects_by(&family, &NamedNode::new(super::RDF_TYPE)?)
        .await?;
    let mut registered_family = false;
    while let Some(row) = family_types.next().await? {
        if row["value"] ==
            Term::NamedNode(NamedNode::new(format!("{GOVERNANCE}Family"))?)
        {
            registered_family = true;
        }
    }
    ensure!(
        registered_family,
        "publication scope must identify an artgov:Family"
    );
    let family_name = english(
        &view,
        &family,
        "http://www.w3.org/2000/01/rdf-schema#label",
    )
    .await?;
    ensure!(
        family_name.len() == 1,
        "registered family requires one English label"
    );
    let family_pattern = view.pattern(
        &format!(
            "?reference <{GOVERNANCE}inFamily> {family} ; \
             <{GOVERNANCE}refersTo> ?ontology"
        ),
        0,
    );
    let direct = view.pattern(
        &format!("?ontology <{GOVERNANCE}inFamily> {family}"),
        1,
    );
    let mut membership = dataset
        .select(format!(
            "SELECT DISTINCT ?reference ?ontology WHERE {{ {{ \
             {family_pattern} }} UNION {{ {direct} BIND(?ontology AS \
             ?reference) }} }}"
        ))
        .await?;
    let mut aliases = HashSet::new();
    let mut references: BTreeMap<NamedNode, HashSet<NamedNode>> =
        BTreeMap::new();
    while let Some(row) = membership.next().await? {
        let Term::NamedNode(iri) = &row["ontology"] else {
            anyhow::bail!("family members must have named artifact identities")
        };
        let Term::NamedNode(reference) = &row["reference"] else {
            anyhow::bail!("family reference must be an IRI")
        };
        references
            .entry(reference.clone())
            .or_default()
            .insert(iri.clone());
        aliases.insert(iri.clone());
    }
    ensure!(
        !aliases.is_empty(),
        "family has no explicit artifact or reference membership; recognition \
         rules are not applied implicitly"
    );
    let mut prefixes = NamespaceBindings::well_known();
    // bounded: 64 aliases per member within the family resource budget.
    let mut aliases_declared = Vec::new();
    let mut members = Vec::new();
    let mut known = HashSet::new();
    let mut scheme_terms = HashSet::new(); // bounded: FAMILY_RESOURCES shared terms.
    let mut term_memberships = 0usize;
    let collected_snapshot = snapshot.clone();
    for processor in sources {
        let snapshot = dataset
            .snapshot_for_processor(project, processor.as_str())
            .await?;
        let source = RevisionView::new(dataset.clone(), snapshot.clone());
        let pattern = source.pattern("?ontology a ?rootKind", 0);
        let owl = source.pattern(
            "?ontology a <http://www.w3.org/2002/07/owl#Ontology>",
            1,
        );
        let scheme_type = source.pattern(
            "?ontology a <http://www.w3.org/2004/02/skos/core#ConceptScheme>",
            2,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?ontology ?rootKind ?scheme WHERE {{ {pattern} FILTER(?rootKind = <http://www.w3.org/2002/07/owl#Ontology> || (?rootKind = <http://www.w3.org/2004/02/skos/core#ConceptScheme> && NOT EXISTS {{ {owl} }})) BIND(EXISTS {{ {scheme_type} }} AS ?scheme) }}"
            ))
            .await?;
        let mut roots = 0usize;
        let mut ontologies = 0usize;
        while let Some(row) = rows.next().await? {
            roots += 1;
            ensure!(
                roots <= super::limits::FAMILY_RESOURCES,
                "source exceeds vocabulary root budget"
            );
            let Term::NamedNode(ontology) = &row["ontology"] else {
                anyhow::bail!("ontology must have a named identity")
            };
            let scheme = row["scheme"] == Literal::from(true).into();
            if row["rootKind"] ==
                NamedNode::new_unchecked(
                    "http://www.w3.org/2002/07/owl#Ontology",
                )
                .into()
            {
                ontologies += 1;
                ensure!(
                    ontologies <= 1,
                    "one source file must declare at most one ontology"
                );
            }
            if !aliases.contains(ontology) {
                continue;
            }
            ensure!(
                known.insert(ontology.clone()),
                "family ontology is declared in multiple source files"
            );
            ensure!(
                known.len() <= super::limits::FAMILY_RESOURCES,
                "family exceeds vocabulary resource budget"
            );
            let declarations = declarations::MemberDeclarations::read(
                dataset, &view, ontology,
            )
            .await?;
            let declarations::Declaration { prefix, namespace } =
                declarations.own;
            prefixes.declare(&namespace, &prefix)?;
            aliases_declared.extend(declarations.aliases);
            // Scheme membership may span files. Read the pinned collected
            // project, not only the declaration file, without reading generated
            // fragments.
            let (source, snapshot) = if scheme {
                (view.clone(), collected_snapshot.clone())
            } else {
                (source.clone(), snapshot.clone())
            };
            let pattern = source.pattern("?subject a ?kind", 0);
            let membership = source.pattern(
                &format!(
                    "?subject <http://www.w3.org/2004/02/skos/core#inScheme> \
                     {ontology}"
                ),
                1,
            );
            let top = source.pattern(
                &format!(
                "?subject <http://www.w3.org/2004/02/skos/core#topConceptOf> \
                 {ontology}"
            ),
                2,
            );
            let has_top = source.pattern(&format!("{ontology} <http://www.w3.org/2004/02/skos/core#hasTopConcept> ?subject"), 3);
            let mut rows = dataset
                .select(format!(
                    "SELECT DISTINCT ?subject WHERE {{ {pattern} \
                     FILTER(isIRI(?subject) && (STRSTARTS(STR(?subject), {}) \
                     || EXISTS {{ {membership} }} || EXISTS {{ {top} }} || \
                     EXISTS {{ {has_top} }}) && ?subject != {ontology}) }} \
                     ORDER BY ?subject",
                    Literal::new_simple_literal(&namespace)
                ))
                .await?;
            let mut terms = Vec::new();
            let mut missing = 0;
            while let Some(row) = rows.next().await? {
                let Term::NamedNode(subject) = &row["subject"] else {
                    anyhow::bail!("family terms require named identities")
                };
                if aliases.contains(subject) {
                    continue;
                }
                ensure!(
                    known.insert(subject.clone()) ||
                        scheme ||
                        scheme_terms.contains(subject),
                    "resource belongs to multiple source namespaces: {subject}"
                );
                if scheme {
                    scheme_terms.insert(subject.clone());
                }
                ensure!(
                    known.len() <= super::limits::FAMILY_RESOURCES,
                    "family exceeds {}-resource publication budget",
                    super::limits::FAMILY_RESOURCES
                );
                term_memberships += 1;
                ensure!(
                    term_memberships <= super::limits::SOURCE_TYPE_ASSERTIONS,
                    "family exceeds vocabulary membership budget"
                );
                let rich =
                    super::document::description::read(&source, subject, "en")
                        .await?;
                let mut descriptions = Vec::new();
                if rich.is_none() {
                    for property in
                        super::document::description::SOURCE_DESCRIPTIONS
                    {
                        descriptions
                            .extend(english(&source, subject, property).await?);
                    }
                }
                if rich.is_none() && descriptions.is_empty() {
                    missing += 1;
                }
                terms.push(subject.clone());
            }
            let labels = vocabulary_labels(&source, ontology).await?;
            ensure!(
                labels.len() == 1 && !labels[0].trim().is_empty(),
                "ontology {ontology} requires one English label for its \
                 chapter title"
            );
            members.push(Member {
                scheme,
                ontology: ontology.clone(),
                prefix,
                title: labels[0].clone(),
                namespace,
                view: source,
                snapshot,
                terms,
                missing,
            });
        }
    }
    for (reference, aliases) in references {
        let loaded = members
            .iter()
            .filter(|member| aliases.contains(&member.ontology))
            .count();
        ensure!(
            loaded > 0,
            "registered family member {reference} has no loaded ontology \
             source"
        );
        ensure!(
            loaded == 1,
            "registered family reference {reference} resolves to multiple \
             source ontologies"
        );
    }
    ensure!(
        !members.is_empty(),
        "no registered family ontology was found in the supplied sources"
    );
    members.sort_by(|a, b| {
        a.prefix.cmp(&b.prefix).then(a.ontology.cmp(&b.ontology))
    });
    let chapters = modules::chapters(dataset, &view, &members).await?;
    let module_count = chapters
        .iter()
        .filter(|chapter| chapter.module_body.is_some())
        .count();
    let ontology_count = members.len();
    let term_count = known.len() - members.len();
    let missing: usize = members.iter().map(|m| m.missing).sum();
    let skos =
        super::document::skos::SchemeArrangement::read(dataset, &view, &known)
            .await?;
    // Aliases apply after every member's own namespace, so they never replace
    // one; the first lexical alias of a namespace wins.
    aliases_declared.sort_by(|a: &declarations::Declaration, b| {
        (&a.namespace, &a.prefix).cmp(&(&b.namespace, &b.prefix))
    });
    for alias in aliases_declared {
        prefixes.declare_alias(&alias.namespace, &alias.prefix)?;
    }
    let prefixes = Arc::new(prefixes.into_map());
    let known = Arc::new(known);
    let owner = identity("family-bindings", &[project, target.as_str()]);
    let contribution = identity("contribution", &[
        project,
        owner.as_str(),
        target.as_str(),
    ]);
    managed.add(owner.as_str(), target.as_str(), true)?;
    let term_processor =
        AnnotatedTermDocumentor::processor_identity(project, "en");
    for member in &members {
        for subject in &member.terms {
            managed.add(term_processor.as_str(), subject.as_str(), true)?;
        }
    }
    managed.persist_inventory(writer, true).await?;
    let mut stream = writer
        .begin_stage(
            project,
            owner.as_str(),
            target.as_str(),
            &format!("family-v12:{snapshot}"),
            super::dataset::Stage::Document,
        )
        .await?;
    let mut data = vec![
        q(&owner, "type", term("Processor")),
        q(&contribution, "type", term("Contribution")),
        q(&contribution, "project", NamedNode::new(project)?),
        q(&contribution, "processor", owner),
        q(
            &contribution,
            "source",
            Literal::new_simple_literal(target.as_str()),
        ),
        q(
            &contribution,
            "revision",
            Literal::new_simple_literal(snapshot.as_str()),
        ),
    ];
    let arrangement =
        arrangement::FamilyArrangement::read(&view, &profile).await?;
    for (order, kind, title, property) in [
        (
            0,
            "abstract",
            "Abstract",
            "http://purl.org/dc/terms/description",
        ),
        (
            1,
            "status",
            "Status of This Document",
            "http://www.w3.org/2000/01/rdf-schema#comment",
        ),
    ]
    .into_iter()
    .filter(|_| arrangement.front_matter)
    {
        let text = english(&view, &target, property).await?;
        ensure!(
            text.len() == 1,
            "family target requires one {property} annotation"
        );
        let subject = identity("family-front-matter", &[target.as_str(), kind]);
        component(
            &mut data, &target, &profile, &subject, title, kind, order,
        );
        fragment(
            &mut data,
            &scope,
            &target,
            &subject,
            &contribution,
            &text[0],
            0,
        );
    }
    let includes_schemes = members.iter().any(|member| member.scheme);
    let (singular, plural, selection, column) = if includes_schemes {
        (
            "vocabulary",
            "vocabularies",
            "a vocabulary",
            "Vocabulary",
        )
    } else {
        (
            "ontology",
            "ontologies",
            "an ontology",
            "Ontology",
        )
    };
    let noun = if ontology_count == 1 {
        singular
    } else {
        plural
    };
    let terminology =
        super::collection::Terminology::read(&view, &profile).await?;
    let collection_term = terminology.singular();
    let mut overview = format!(
        "# {}\n\nExplore the {ontology_count} {noun} in this \
         {collection_term}. Select {selection} to read its vocabulary \
         reference.\n\n",
        family_name[0]
    );
    let mut coverage = format!(
        "{ontology_count} {noun}; {term_count} documented named resources. \
         {missing} resources have no English source definition.\n\nMissing \
         annotations are shown explicitly and recorded as KG diagnostics. No \
         inferred definitions or external imports are added.\n\n| {column} | \
         Resources | Missing definitions |\n| --- | ---: | ---: |\n"
    );
    if module_count > 0 {
        overview.push_str(&format!(
            "## Modules\n\n{module_count} explicitly declared modules. \
             Ontologies without explicit module membership remain separate \
             reference chapters.\n\n"
        ));
        for chapter in &chapters {
            if chapter.module_body.is_some() {
                overview.push_str(&format!(
                    "{}- [{}]({})\n",
                    "  ".repeat(chapter.depth - 1),
                    chapter.title,
                    chapter.subject.as_str()
                ));
            }
        }
    }
    for member in &members {
        if module_count == 0 {
            overview.push_str(&format!(
                "- **[{}]({})**\n",
                member.title,
                member.ontology.as_str(),
            ));
            let descriptions = english(
                &member.view,
                &member.ontology,
                "http://www.w3.org/2000/01/rdf-schema#comment",
            )
            .await?;
            if let Some(description) = descriptions.first() {
                // Keep supplied prose; indent continuation lines within the
                // list item.
                overview.push_str(&format!(
                    "\n  {}\n",
                    description.replace('\n', "\n  ")
                ));
            }
            overview.push('\n');
        }
        coverage.push_str(&format!(
            "| [{}]({}) | {} | {} |\n",
            member.title,
            member.ontology.as_str(),
            member.terms.len(),
            member.missing
        ));
    }
    {
        component(
            &mut data,
            &target,
            &profile,
            &family,
            &terminology.overview(),
            "section",
            arrangement.overview_order(),
        );
        arrangement.nest_overview(&mut data, &target, &family);
        arrangement::role(&mut data, &target, &family, arrangement::OVERVIEW);
        fragment(
            &mut data,
            &scope,
            &target,
            &family,
            &contribution,
            &overview,
            0,
        );
    }
    // bounded: each selected resource has one canonical full entry, even when
    // several registered SKOS schemes share its namespace and membership.
    let mut placed_terms = HashSet::new();
    for (order, chapter) in chapters.iter().enumerate() {
        if let Some(body) = &chapter.module_body {
            component(
                &mut data,
                &target,
                &profile,
                &chapter.subject,
                &chapter.title,
                "section",
                arrangement.chapter(order),
            );
            if let Some(parent) = &chapter.parent {
                parent_placement(&mut data, &target, &chapter.subject, parent);
            } else {
                arrangement.nest(&mut data, &target, &chapter.subject);
            }
            if let Some(slug) =
                authored_page_slug(&view, &chapter.subject).await?
            {
                page_slug(&mut data, &target, &chapter.subject, &slug);
            }
            fragment(
                &mut data,
                &scope,
                &target,
                &chapter.subject,
                &contribution,
                body,
                0,
            );
            continue;
        }
        let member = &members[chapter.member.expect("ontology chapter index")];
        if let Some(parent) = &chapter.parent {
            parent_placement(&mut data, &target, &chapter.subject, parent);
        } else {
            arrangement.nest(&mut data, &target, &member.ontology);
        }
        component(
            &mut data,
            &target,
            &profile,
            &member.ontology,
            &member.title,
            "section",
            arrangement.chapter(order),
        );
        // An ontology page is named by the namespace prefix its family binds,
        // unless the ontology declares its own page slug.
        let slug = authored_page_slug(&view, &member.ontology)
            .await?
            .unwrap_or_else(|| member.prefix.clone());
        page_slug(&mut data, &target, &member.ontology, &slug);
        let labels = vocabulary_labels(&member.view, &member.ontology).await?;
        let description =
            reference::introduction(&member.view, &member.ontology).await?;
        let layout =
            reference::Layout::read(&member.view, &member.ontology).await?;
        let root_label = if member.scheme {
            "Concept scheme IRI"
        } else {
            "Ontology IRI"
        };
        let identifiers = if member.ontology.as_str() == member.namespace {
            format!(
                "| {root_label} / Namespace | `{}` |\n",
                member.namespace
            )
        } else {
            format!(
                "| {root_label} | `{}` |\n| Namespace | `{}` |\n",
                member.ontology.as_str(),
                member.namespace
            )
        };
        if member.scheme {
            let decoration = skos.decoration(&member.ontology, 1);
            data.extend(decoration.diagnostics(
                &super::diagnostic_graph::Context {
                    graph:     NamedNode::new_unchecked(
                        "urn:grapharch:quality",
                    ),
                    id:        contribution.clone(),
                    project:   NamedNode::new(project)?,
                    about:     member.ontology.clone(),
                    revision:  snapshot.clone(),
                    processor: identity("family-bindings", &[
                        project,
                        target.as_str(),
                    ]),
                    source:    member.ontology.as_str().into(),
                },
            )?);
        }
        let scheme_navigation = if member.scheme {
            skos.decoration(&member.ontology, 1).body
        } else {
            String::new()
        };
        let introduction = format!(
            "## {}\n\n{}\n\n| Identifier | Value |\n| --- | --- |\n{}| Prefix \
             | `{}` |\n\n{}{}\n",
            layout.overview,
            labels.join("; "),
            identifiers,
            member.prefix,
            description,
            scheme_navigation
        );
        fragment(
            &mut data,
            &scope,
            &target,
            &member.ontology,
            &contribution,
            &introduction,
            0,
        );
        let groups = layout.group(dataset, &member.view, &member.terms).await?;
        let mut arranged_groups = Vec::new();
        for (section, terms) in groups {
            if section.kind == "concepts" {
                let (unassigned, assigned): (Vec<_>, Vec<_>) =
                    terms.into_iter().partition(|subject| {
                        skos.canonical_scheme(subject).is_none()
                    });
                let assigned = assigned
                    .into_iter()
                    .filter(|subject| {
                        skos.canonical_scheme(subject) == Some(&member.ontology)
                    })
                    .collect::<Vec<_>>();
                if !assigned.is_empty() {
                    arranged_groups.push((section.clone(), assigned));
                }
                if !unassigned.is_empty() {
                    let mut unassigned_section = section;
                    unassigned_section.kind = "unassigned-concepts".into();
                    unassigned_section.title =
                        "Concepts without a documented scheme".into();
                    arranged_groups.push((unassigned_section, unassigned));
                }
                continue;
            }
            let mut arranged = Vec::new();
            for subject in terms {
                if skos.is_concept(&subject) {
                    continue;
                }
                arranged.push(subject.clone());
                if skos.is_scheme(&subject) {
                    arranged.extend(skos.canonical_members(&subject));
                }
            }
            if !arranged.is_empty() {
                arranged_groups.push((section, arranged));
            }
        }
        let mut position = 1u32;
        for (section, terms) in arranged_groups {
            let terms = terms
                .into_iter()
                .filter(|subject| placed_terms.insert(subject.clone()))
                .collect::<Vec<_>>();
            if terms.is_empty() {
                continue;
            }
            let id = identity("family-reference-section", &[
                target.as_str(),
                member.ontology.as_str(),
                &section.kind,
            ]);
            data.extend([
                q(
                    &identity("family-component", &[
                        target.as_str(),
                        member.ontology.as_str(),
                    ]),
                    "fragment",
                    id.clone(),
                ),
                q(&id, "type", term("Fragment")),
                q(&id, "subject", member.ontology.clone()),
                q(
                    &id,
                    "body",
                    Literal::new_simple_literal(format!(
                        "## {}\n",
                        section.title
                    )),
                ),
                q(
                    &id,
                    "format",
                    Literal::new_simple_literal("text/markdown"),
                ),
                q(&id, "contribution", contribution.clone()),
                q(&id, "order", Literal::from(position)),
            ]);
            position += 1;
            for subject in terms {
                let documentor = AnnotatedTermDocumentor::new(
                    dataset.clone(),
                    member.snapshot.clone(),
                    project,
                    subject.clone(),
                    "en",
                    prefixes.clone(),
                    known.clone(),
                )?
                .with_family_reference();
                data.extend([
                    q(&scope, "include", subject.clone()),
                    q(
                        &identity("family-component", &[
                            target.as_str(),
                            member.ontology.as_str(),
                        ]),
                        "fragment",
                        documentor.fragment_id(),
                    ),
                    q(
                        &documentor.fragment_id(),
                        "order",
                        Literal::from(position),
                    ),
                ]);
                position += 1;
            }
        }
    }
    {
        let coverage_subject = identity("family-coverage", &[target.as_str()]);
        arrangement::role(
            &mut data,
            &target,
            &coverage_subject,
            arrangement::COVERAGE,
        );
        component(
            &mut data,
            &target,
            &profile,
            &coverage_subject,
            "Documentation coverage",
            "appendix",
            arrangement.coverage_order(chapters.len()),
        );
        fragment(
            &mut data,
            &scope,
            &target,
            &coverage_subject,
            &contribution,
            &coverage,
            0,
        );
    }
    for quad in data {
        stream.write(quad).await?;
    }
    stream.complete().await?;
    let root_schemes = members
        .iter()
        .filter(|member| member.scheme)
        .map(|member| member.ontology.clone())
        .collect::<HashSet<_>>();
    let mut scheduled = HashSet::new();
    let mut jobs = Vec::new(); // bounded: one job per placed FAMILY_RESOURCES subject.
    for member in &members {
        for subject in &member.terms {
            if !placed_terms.contains(subject) ||
                !scheduled.insert(subject.clone())
            {
                continue;
            }
            let root_member = skos
                .canonical_scheme(subject)
                .is_some_and(|scheme| root_schemes.contains(scheme));
            jobs.push((
                member.snapshot.clone(),
                subject.clone(),
                (!member.scheme).then(|| member.ontology.clone()),
                skos.decoration(subject, if root_member { 1 } else { 2 }),
            ));
        }
    }
    let term_processor =
        AnnotatedTermDocumentor::processor_identity(project, "en");
    for (_, subject, _, _) in &jobs {
        managed.add(term_processor.as_str(), subject.as_str(), true)?;
    }
    let diagram_renderer = Arc::new(
        crate::publication::document::plantuml::LocalRenderer::new(
            std::ffi::OsStr::new("plantuml"),
        ),
    );
    futures::stream::iter(jobs.into_iter().map(
        |(snapshot, subject, ontology, arrangement)| {
            let dataset = dataset.clone();
            let prefixes = prefixes.clone();
            let known = known.clone();
            let diagram_renderer = diagram_renderer.clone();
            let diagram_metadata = collected_snapshot.clone();
            async move {
                let documentor = AnnotatedTermDocumentor::new(
                    dataset, snapshot, project, subject, "en", prefixes, known,
                )?
                .with_diagram_renderer(diagram_renderer)
                .with_diagram_metadata(diagram_metadata)
                .with_family_reference()
                .with_arrangement(arrangement);
                let documentor = if let Some(ontology) = ontology {
                    documentor.with_ontology_context(ontology)
                } else {
                    documentor
                };
                documentor.document(writer).await
            }
        },
    ))
    .buffer_unordered(concurrency)
    .try_collect::<Vec<_>>()
    .await?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        ontology_count,
        term_count,
        "family resources documented"
    );
    managed.finish(writer).await?;
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        "family revision assembled"
    );
    let plans = plan_targets(
        dataset, writer, project, &revision, &profile, targets,
    )
    .await?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        targets = plans.len(),
        "family targets planned"
    );
    Ok((plans, ontology_count, term_count))
}

/// Plan every selected target from the one assembled revision. The family
/// bindings belong to the primary target's profile, so each other target must
/// select that profile.
async fn plan_targets(
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    revision: &NamedNode,
    profile: &NamedNode,
    targets: &FamilyTargets,
) -> Result<Vec<ResolvedPlan>> {
    let view = RevisionView::new(dataset.clone(), revision.clone());
    let planner = TargetPlanner::new(dataset.clone(), revision.clone());
    // bounded: a selection names at most 16 targets.
    let mut plans = Vec::with_capacity(targets.selected().len());
    for target in targets.selected() {
        let used = view.node(target, "profile").await?;
        ensure!(
            &used == profile,
            "target {target} uses profile {used}, not the family profile \
             {profile}"
        );
        plans.push(planner.resolve(project, target, writer).await?);
    }
    Ok(plans)
}
async fn vocabulary_labels(
    view: &RevisionView,
    subject: &NamedNode,
) -> Result<Vec<String>> {
    let preferred = english(
        view,
        subject,
        "http://www.w3.org/2004/02/skos/core#prefLabel",
    )
    .await?;
    ensure!(
        preferred.len() <= 1,
        "{subject} has ambiguous English skos:prefLabel annotations"
    );
    if !preferred.is_empty() {
        return Ok(preferred);
    }
    for predicate in [
        "http://www.w3.org/2000/01/rdf-schema#label",
        "http://www.w3.org/ns/shacl#name",
        "http://purl.org/dc/terms/title",
        "http://purl.org/dc/elements/1.1/title",
    ] {
        let labels = english(view, subject, predicate).await?;
        if !labels.is_empty() {
            return Ok(labels);
        }
    }
    Ok(Vec::new())
}

async fn english(
    view: &RevisionView,
    subject: &NamedNode,
    predicate: &str,
) -> Result<Vec<String>> {
    let mut rows = view
        .objects_by(subject, &NamedNode::new(predicate)?)
        .await?;
    let mut values = Vec::new();
    while let Some(row) = rows.next().await? {
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!("text annotation must be literal")
        };
        if value
            .language()
            .is_none_or(|language| language.eq_ignore_ascii_case("en"))
        {
            values.push(value.value().to_owned());
        }
    }
    values.sort();
    values.dedup();
    Ok(values)
}
fn q(subject: &NamedNode, field: &str, object: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        if field == "type" {
            NamedNode::new_unchecked(super::RDF_TYPE)
        } else {
            term(field)
        },
        object,
        GraphName::DefaultGraph,
    )
}
fn parent_placement(
    data: &mut Vec<Quad>,
    target: &NamedNode,
    subject: &NamedNode,
    parent: &NamedNode,
) {
    data.push(q(
        &identity("family-placement", &[
            target.as_str(),
            subject.as_str(),
        ]),
        "parentPlacement",
        identity("family-placement", &[
            target.as_str(),
            parent.as_str(),
        ]),
    ));
}
/// The page slug declared on a documented ontology or module, if any.
async fn authored_page_slug(
    view: &RevisionView,
    subject: &NamedNode,
) -> Result<Option<String>> {
    match view.optional(subject, "pageSlug").await? {
        None => Ok(None),
        Some(Term::Literal(slug)) => Ok(Some(slug.value().to_owned())),
        Some(_) => anyhow::bail!("ga:pageSlug of {subject} must be a literal"),
    }
}
/// Record the page slug of a subject's placement for paginated renderers.
fn page_slug(
    data: &mut Vec<Quad>,
    target: &NamedNode,
    subject: &NamedNode,
    slug: &str,
) {
    data.push(q(
        &identity("family-placement", &[
            target.as_str(),
            subject.as_str(),
        ]),
        "pageSlug",
        Literal::new_simple_literal(slug),
    ));
}
fn component(
    data: &mut Vec<Quad>,
    target: &NamedNode,
    profile: &NamedNode,
    subject: &NamedNode,
    title: &str,
    kind: &str,
    order: u32,
) {
    let placement = identity("family-placement", &[
        target.as_str(),
        subject.as_str(),
    ]);
    let component = identity("family-component", &[
        target.as_str(),
        subject.as_str(),
    ]);
    data.extend([
        q(profile, "placement", placement.clone()),
        q(&placement, "type", term("Placement")),
        q(&placement, "component", component.clone()),
        q(&placement, "order", Literal::from(order)),
        q(
            &placement,
            "sectionKind",
            Literal::new_simple_literal(kind),
        ),
        q(&component, "type", term("Component")),
    ]);
    data.push(Quad::new(
        component,
        NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
        Literal::new_simple_literal(title),
        GraphName::DefaultGraph,
    ));
}
fn fragment(
    data: &mut Vec<Quad>,
    scope: &NamedNode,
    target: &NamedNode,
    subject: &NamedNode,
    contribution: &NamedNode,
    body: &str,
    order: u32,
) {
    let fragment = identity(
        "family-introduction-fragment",
        &[subject.as_str()],
    );
    data.extend([
        q(scope, "include", subject.clone()),
        q(
            &identity("family-component", &[
                target.as_str(),
                subject.as_str(),
            ]),
            "fragment",
            fragment.clone(),
        ),
        q(&fragment, "type", term("Fragment")),
        q(&fragment, "subject", subject.clone()),
        q(
            &fragment,
            "body",
            Literal::new_simple_literal(body),
        ),
        q(
            &fragment,
            "format",
            Literal::new_simple_literal("text/markdown"),
        ),
        q(&fragment, "contribution", contribution.clone()),
        q(&fragment, "order", Literal::from(order)),
    ]);
}

// The manifest records only this builder's ownership; it never scans/removes
// unrelated project contributors. Failed discovery retains a failed attempt.
struct ManagedFamily {
    project:        String,
    owner:          NamedNode,
    inventory:      NamedNode,
    target:         String,
    previous:       std::collections::BTreeSet<(String, String, bool)>,
    last_inventory: std::collections::BTreeSet<(String, String, bool)>,
    next:           std::collections::BTreeSet<(String, String, bool)>,
    stream:         Option<super::writer::ContributionStream>,
}
impl ManagedFamily {
    async fn begin(
        dataset: &Dataset,
        writer: &GraphWriter,
        project: &str,
        target: &str,
    ) -> Result<Self> {
        let owner = identity("family-discovery", &[project, target]);
        let inventory = identity("family-inventory", &[project, target]);
        let snapshot = dataset.snapshot_for(project).await?;
        let view = RevisionView::new(dataset.clone(), snapshot);
        let pattern = view.pattern(&format!(
            "VALUES ?root {{ {owner} {inventory} }} ?root <http://www.w3.org/ns/prov#hadMember> ?entry . OPTIONAL {{ ?entry <http://www.w3.org/ns/prov#wasAttributedTo> ?processor ; <http://purl.org/dc/terms/identifier> ?source ; <http://purl.org/dc/terms/description> ?stage }}"
        ), 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?root ?processor ?source ?stage WHERE {{ \
                 {pattern} }} LIMIT 200001"
            ))
            .await?;
        let mut previous = std::collections::BTreeSet::new();
        let mut last_inventory = std::collections::BTreeSet::new();
        while let Some(row) = rows.next().await? {
            let Some(Term::NamedNode(processor)) = row.get("processor") else {
                anyhow::bail!("missing or invalid manifest processor");
            };
            let Some(Term::Literal(source)) = row.get("source") else {
                anyhow::bail!("missing or invalid manifest source");
            };
            let Some(Term::Literal(stage)) = row.get("stage") else {
                anyhow::bail!("missing or invalid manifest stage");
            };
            ensure!(
                matches!(stage.value(), "collect" | "document"),
                "invalid manifest stage"
            );
            let entry = (
                processor.as_str().into(),
                source.value().into(),
                stage.value() == "document",
            );
            if row.get("root") == Some(&Term::NamedNode(inventory.clone())) {
                last_inventory.insert(entry.clone());
            }
            previous.insert(entry);
            ensure!(
                previous.len() <= 100_000,
                "family ownership manifest exceeds entry budget"
            );
        }
        let stream = writer
            .begin_stage(
                project,
                owner.as_str(),
                target,
                &format!("pending:{}", uuid::Uuid::new_v4()),
                super::dataset::Stage::Document,
            )
            .await?;
        Ok(Self {
            project: project.into(),
            owner,
            inventory,
            target: target.into(),
            previous,
            last_inventory,
            next: Default::default(),
            stream: Some(stream),
        })
    }

    fn add(
        &mut self,
        processor: &str,
        source: &str,
        document: bool,
    ) -> Result<()> {
        ensure!(
            self.next.len() < 100_000,
            "family ownership manifest exceeds entry budget"
        );
        self.next
            .insert((processor.into(), source.into(), document));
        Ok(())
    }

    /// Persist ownership before producers can acknowledge any inputs. Keep
    /// previous entries until success, so cancellation and failure cannot
    /// orphan either old or newly discovered contributions.
    async fn persist_inventory(
        &mut self,
        writer: &GraphWriter,
        include_previous: bool,
    ) -> Result<()> {
        use sha2::{Digest, Sha256};
        let mut entries = self.next.clone();
        if include_previous {
            entries.extend(self.previous.iter().cloned());
        }
        ensure!(
            entries.len() <= 100_000,
            "family ownership inventory exceeds entry budget"
        );
        if entries == self.last_inventory {
            return Ok(());
        }
        let mut hash = Sha256::new();
        for entry in &entries {
            hash.update(format!("{entry:?}\n"));
        }
        let revision = format!(
            "family-inventory-v1:{}",
            hex::encode(hash.finalize())
        );
        let mut stream = writer
            .begin_stage(
                &self.project,
                self.inventory.as_str(),
                &self.target,
                &revision,
                super::dataset::Stage::Document,
            )
            .await?;
        write_owned_entries(&mut stream, &self.inventory, &entries).await?;
        stream.complete().await?;
        self.last_inventory = entries;
        Ok(())
    }

    async fn retire(&self, writer: &GraphWriter, document: bool) -> Result<()> {
        for (processor, source, stage) in self.previous.difference(&self.next) {
            if *stage == document {
                writer.remove(&self.project, processor, source).await?;
            }
        }
        Ok(())
    }

    async fn finish(&mut self, writer: &GraphWriter) -> Result<()> {
        use sha2::{Digest, Sha256};
        self.retire(writer, true).await?;
        let mut hash = Sha256::new();
        for entry in &self.next {
            hash.update(format!("{entry:?}\n"));
        }
        let revision = format!(
            "family-manifest-v1:{}",
            hex::encode(hash.finalize())
        );
        let stream = self.stream.as_mut().unwrap();
        // Pending streams created with an explicit document stage need a final
        // revision just as collected streams do.
        stream.set_revision(&revision).await?;
        write_owned_entries(stream, &self.owner, &self.next).await?;
        self.persist_inventory(writer, false).await?;
        self.stream.take().unwrap().complete().await
    }

    async fn abort(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            stream.abort().await?;
        }
        Ok(())
    }
}

async fn write_owned_entries(
    stream: &mut super::writer::ContributionStream,
    root: &NamedNode,
    entries: &std::collections::BTreeSet<(String, String, bool)>,
) -> Result<()> {
    stream
        .write(Quad::new(
            root.clone(),
            NamedNode::new(super::RDF_TYPE)?,
            NamedNode::new("http://www.w3.org/ns/prov#Collection")?,
            GraphName::DefaultGraph,
        ))
        .await?;
    for (processor, source, document) in entries {
        let entry = identity("family-owned-input", &[
            root.as_str(),
            processor,
            source,
        ]);
        for (subject, predicate, object) in [
            (
                root.clone(),
                "http://www.w3.org/ns/prov#hadMember",
                Term::from(entry.clone()),
            ),
            (
                entry.clone(),
                "http://www.w3.org/ns/prov#wasAttributedTo",
                NamedNode::new(processor)?.into(),
            ),
            (
                entry.clone(),
                "http://purl.org/dc/terms/identifier",
                Literal::new_simple_literal(source).into(),
            ),
            (
                entry,
                "http://purl.org/dc/terms/description",
                Literal::new_simple_literal(
                    if *document { "document" } else { "collect" },
                )
                .into(),
            ),
        ] {
            stream
                .write(Quad::new(
                    subject,
                    NamedNode::new(predicate)?,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
    }
    Ok(())
}
