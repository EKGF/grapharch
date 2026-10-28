//! A registry-selected family is collected once and published from one pinned
//! KG.
use {
    super::{
        assembly::Assembler,
        collect::{Collector, RdfFile, collect_all},
        dataset::{Dataset, identity},
        document::{AnnotatedTermDocumentor, Documentor},
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
mod modules;
mod reference;

const GOVERNANCE: &str = "https://ekgf.org/ontology/artifact-governance#";
const PUBLICATION: &str = "https://ekgf.org/ontology/artifact-publication#";
pub struct CollectedFamily {
    pub dataset:        Dataset,
    pub plan:           ResolvedPlan,
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
        target,
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
        &repository.target,
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
    concurrency: usize,
) -> Result<CollectedFamily> {
    match source {
        FamilySource::RemoteRepository { url, reference, cache } => {
            let repository = super::collect::git::remote_publication(
                url, reference, cache, hints_path,
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
            collect_from_optional(
                dataset,
                source,
                None,
                &hints.project,
                &hints.target,
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
        FamilySource::Sparql(_) => {
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
    target: &str,
    concurrency: usize,
) -> Result<CollectedFamily> {
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
            target,
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
    let (plan, ontology_count, term_count) = result?;
    Ok(CollectedFamily { dataset, plan, ontology_count, term_count })
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
    target: &str,
    concurrency: usize,
    managed: &mut ManagedFamily,
) -> Result<(ResolvedPlan, usize, usize)> {
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
    managed.retire(writer, false).await?;
    collect_all(work, writer, concurrency).await?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        "family sources collected"
    );
    let snapshot = dataset.snapshot_collected_for(project).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    let target = NamedNode::new(target)?;
    ensure!(
        matches!(
            view.text(&target, "renderer").await?.as_str(),
            "specification" | "specification-pages" | "bookshelf"
        ),
        "family target requires a specification renderer"
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
    let mut prefixes = standard_prefixes();
    let mut members = Vec::new();
    let mut known = HashSet::new();
    let collected_snapshot = snapshot.clone();
    for processor in sources {
        let snapshot = dataset
            .snapshot_for_processor(project, processor.as_str())
            .await?;
        let source = RevisionView::new(dataset.clone(), snapshot.clone());
        let pattern = source.pattern("?ontology a ?rootKind", 0);
        let owl = source.pattern(
            "?owlRoot a <http://www.w3.org/2002/07/owl#Ontology>",
            1,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?ontology ?rootKind WHERE {{ {pattern} FILTER(?rootKind = <http://www.w3.org/2002/07/owl#Ontology> || (?rootKind = <http://www.w3.org/2004/02/skos/core#ConceptScheme> && NOT EXISTS {{ {owl} }})) }}"
            ))
            .await?;
        let Some(row) = rows.next().await? else {
            continue;
        };
        ensure!(
            rows.next().await?.is_none(),
            "one source file must declare at most one ontology or standalone \
             concept scheme"
        );
        let Term::NamedNode(ontology) = &row["ontology"] else {
            anyhow::bail!("ontology must have a named identity")
        };
        let scheme = row["rootKind"] ==
            NamedNode::new_unchecked(
                "http://www.w3.org/2004/02/skos/core#ConceptScheme",
            )
            .into();
        if !aliases.contains(ontology) {
            continue;
        }
        ensure!(
            known.insert(ontology.clone()),
            "family ontology is declared in multiple source files"
        );
        let declaration=view.pattern(&format!("{ontology} <http://www.w3.org/ns/shacl#declare> ?declaration . ?declaration <http://www.w3.org/ns/shacl#prefix> ?prefix ; <http://www.w3.org/ns/shacl#namespace> ?namespace"),0);
        let mut names = dataset
            .select(format!(
                "SELECT DISTINCT ?prefix ?namespace WHERE {{ {declaration} }}"
            ))
            .await?;
        let row = names.next().await?.ok_or_else(|| {
            anyhow::anyhow!(
                "ontology {ontology} requires explicit sh:declare namespace \
                 metadata"
            )
        })?;
        ensure!(
            names.next().await?.is_none(),
            "ontology namespace declaration is ambiguous"
        );
        let (Term::Literal(prefix), Term::Literal(namespace)) =
            (&row["prefix"], &row["namespace"])
        else {
            anyhow::bail!("namespace metadata must be literal")
        };
        let (prefix, namespace) = (
            prefix.value().to_owned(),
            namespace.value().to_owned(),
        );
        ensure!(
            !prefix.is_empty() && !namespace.is_empty(),
            "namespace metadata cannot be empty"
        );
        ensure!(
            !prefixes.values().any(|existing| existing == &prefix),
            "family namespace prefix {prefix} is ambiguous"
        );
        prefixes.insert(namespace.clone(), prefix.clone());
        // Scheme membership may span files. Read the pinned collected project,
        // not only the declaration file, without reading generated fragments.
        let (source, snapshot) = if scheme {
            (view.clone(), collected_snapshot.clone())
        } else {
            (source, snapshot)
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
                 FILTER(isIRI(?subject) && (STRSTARTS(STR(?subject), {}) || \
                 EXISTS {{ {membership} }} || EXISTS {{ {top} }} || EXISTS {{ \
                 {has_top} }}) && ?subject != {ontology}) }} ORDER BY ?subject",
                Literal::new_simple_literal(&namespace)
            ))
            .await?;
        let mut terms = Vec::new();
        let mut missing = 0;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(subject) = &row["subject"] else {
                anyhow::bail!("family terms require named identities")
            };
            ensure!(
                known.insert(subject.clone()),
                "resource belongs to multiple source namespaces: {subject}"
            );
            ensure!(
                known.len() <= super::limits::FAMILY_RESOURCES,
                "family exceeds {}-resource publication budget",
                super::limits::FAMILY_RESOURCES
            );
            let rich =
                super::document::description::read(&source, subject, "en")
                    .await?;
            let mut descriptions = Vec::new();
            if rich.is_none() {
                for property in [
                    "http://www.w3.org/2000/01/rdf-schema#comment",
                    "http://purl.org/dc/terms/description",
                    "http://www.w3.org/2004/02/skos/core#definition",
                ] {
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
            "ontology {ontology} requires one English label for its chapter \
             title"
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
    members.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    let chapters = modules::chapters(dataset, &view, &members).await?;
    let module_count = chapters
        .iter()
        .filter(|chapter| chapter.module_body.is_some())
        .count();
    let ontology_count = members.len();
    let term_count = members.iter().map(|m| m.terms.len()).sum();
    let missing: usize = members.iter().map(|m| m.missing).sum();
    let prefixes = Arc::new(prefixes);
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
            &format!("family-v10:{snapshot}"),
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
    ] {
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
    let mut overview = format!(
        "# {}\n\nExplore the {ontology_count} ontologies in this family. \
         Select an ontology to read its vocabulary reference.\n\n",
        family_name[0]
    );
    let mut coverage = format!(
        "{ontology_count} ontologies; {term_count} documented named \
         resources. {missing} resources have no English source \
         definition.\n\nMissing annotations are shown explicitly and recorded \
         as KG diagnostics. No inferred definitions or external imports are \
         added.\n\n| Ontology | Resources | Missing definitions |\n| --- | \
         ---: | ---: |\n"
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
    component(
        &mut data,
        &target,
        &profile,
        &family,
        "Family overview",
        "section",
        2,
    );
    fragment(
        &mut data,
        &scope,
        &target,
        &family,
        &contribution,
        &overview,
        0,
    );
    for (order, chapter) in chapters.iter().enumerate() {
        if let Some(body) = &chapter.module_body {
            component(
                &mut data,
                &target,
                &profile,
                &chapter.subject,
                &chapter.title,
                "section",
                order as u32 + 3,
            );
            if let Some(parent) = &chapter.parent {
                parent_placement(&mut data, &target, &chapter.subject, parent);
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
        }
        component(
            &mut data,
            &target,
            &profile,
            &member.ontology,
            &member.title,
            "section",
            order as u32 + 3,
        );
        let mut comments = english(
            &member.view,
            &member.ontology,
            "http://www.w3.org/2000/01/rdf-schema#comment",
        )
        .await?;
        for predicate in [
            "http://www.w3.org/2004/02/skos/core#definition",
            "http://purl.org/dc/terms/description",
        ] {
            comments.extend(
                english(&member.view, &member.ontology, predicate).await?,
            );
        }
        let labels = vocabulary_labels(&member.view, &member.ontology).await?;
        let description = match super::document::description::read(
            &member.view,
            &member.ontology,
            "en",
        )
        .await?
        {
            Some(body) => super::document::description::nested(&body, 3)?,
            None => comments.join("\n\n"),
        };
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
        let introduction = format!(
            "## {}\n\n{}\n\n| Identifier | Value |\n| --- | --- |\n{}| Prefix \
             | `{}` |\n\n{}\n",
            layout.overview,
            labels.join("; "),
            identifiers,
            member.prefix,
            description
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
        let mut position = 1u32;
        for (section, terms) in groups {
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
    let coverage_subject = identity("family-coverage", &[target.as_str()]);
    component(
        &mut data,
        &target,
        &profile,
        &coverage_subject,
        "Documentation coverage",
        "appendix",
        chapters.len() as u32 + 3,
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
    for quad in data {
        stream.write(quad).await?;
    }
    stream.complete().await?;
    let jobs = members
        .iter()
        .flat_map(|member| {
            member
                .terms
                .iter()
                .map(|subject| (member.snapshot.clone(), subject.clone()))
        })
        .collect::<Vec<_>>();
    let term_processor =
        AnnotatedTermDocumentor::processor_identity(project, "en");
    for (_, subject) in &jobs {
        managed.add(term_processor.as_str(), subject.as_str(), true)?;
    }
    futures::stream::iter(jobs.into_iter().map(|(snapshot, subject)| {
        let dataset = dataset.clone();
        let prefixes = prefixes.clone();
        let known = known.clone();
        async move {
            AnnotatedTermDocumentor::new(
                dataset, snapshot, project, subject, "en", prefixes, known,
            )?
            .with_family_reference()
            .document(writer)
            .await
        }
    }))
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
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(project, &target, writer)
        .await?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        "family target planned"
    );
    Ok((plan, ontology_count, term_count))
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
    english(
        view,
        subject,
        "http://www.w3.org/2000/01/rdf-schema#label",
    )
    .await
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
fn standard_prefixes() -> BTreeMap<String, String> {
    [
        (
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdf",
        ),
        ("http://www.w3.org/2000/01/rdf-schema#", "rdfs"),
        ("http://www.w3.org/2002/07/owl#", "owl"),
        ("http://www.w3.org/2001/XMLSchema#", "xsd"),
        ("http://www.w3.org/ns/shacl#", "sh"),
        ("http://www.w3.org/2004/02/skos/core#", "skos"),
        ("http://purl.org/dc/terms/", "dct"),
        (
            "https://www.omg.org/spec/Commons/PartiesAndSituations/",
            "cmns-pts",
        ),
        (
            "https://www.omg.org/spec/Commons/RolesAndCompositions/",
            "cmns-rlcmp",
        ),
        (
            "https://www.omg.org/spec/Commons/Organizations/",
            "cmns-org",
        ),
        (
            "https://www.omg.org/spec/Commons/Classifiers/",
            "cmns-cls",
        ),
        (
            "https://www.omg.org/spec/Commons/Designators/",
            "cmns-dsg",
        ),
    ]
    .into_iter()
    .map(|(namespace, prefix)| (namespace.into(), prefix.into()))
    .collect()
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
