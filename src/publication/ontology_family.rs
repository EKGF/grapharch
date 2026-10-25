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
    prefix:    String,
    title:     String,
    namespace: String,
    view:      RevisionView,
    snapshot:  NamedNode,
    terms:     Vec<NamedNode>,
    missing:   usize,
}

pub async fn collect(
    source: &Path,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedFamily> {
    let source = tokio::fs::canonicalize(source).await?;
    let definitions = tokio::fs::canonicalize(definitions).await?;
    let inputs = file_collectors(&source, &definitions, project).await?;
    collect_inputs(inputs, &definitions, project, target, concurrency).await
}

/// Read a Git checkout directly, including its uncommitted working-tree edits.
pub async fn collect_repository(
    source: &Path,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedFamily> {
    let scanned =
        super::collect::git::local_repository(source, project).await?;
    let definitions = tokio::fs::canonicalize(definitions).await?;
    let mut inputs =
        file_collectors(&scanned.root, &definitions, project).await?;
    inputs.push(scanned.provenance);
    collect_inputs(inputs, &definitions, project, target, concurrency).await
}

/// Publish one pinned remote Git revision, reading blobs from its bare cache.
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
    let definitions = tokio::fs::canonicalize(definitions).await?;
    let scanned =
        super::collect::git::remote_repository(url, reference, cache, project)
            .await?;
    let mut inputs = scanned.files;
    inputs.push(scanned.provenance);
    collect_inputs(inputs, &definitions, project, target, concurrency).await
}
async fn file_collectors(
    root: &Path,
    definitions: &Path,
    project: &str,
) -> Result<Vec<Arc<dyn Collector>>> {
    let mut inputs: Vec<Arc<dyn Collector>> = Vec::new();
    for path in discover(root).await? {
        if path == definitions {
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
async fn collect_inputs(
    inputs: Vec<Arc<dyn Collector>>,
    definitions: &Path,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<CollectedFamily> {
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 16, 4 * 1024 * 1024)?;
    let result = build(
        inputs,
        definitions,
        &dataset,
        &writer,
        project,
        target,
        concurrency,
    )
    .await;
    drop(writer);
    worker.await??;
    let (plan, ontology_count, term_count) = result?;
    Ok(CollectedFamily { dataset, plan, ontology_count, term_count })
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
    definitions: &Path,
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    target: &str,
    concurrency: usize,
) -> Result<(ResolvedPlan, usize, usize)> {
    let started = std::time::Instant::now();
    let mut work: Vec<Arc<dyn Collector>> = vec![Arc::new(RdfFile::new(
        project,
        "urn:grapharch:family-definitions",
        definitions.to_owned(),
    )?)];
    let mut sources = Vec::new();
    for collector in inputs {
        sources.push(NamedNode::new(collector.identity()?.processor)?);
        work.push(collector);
    }
    collect_all(work, writer, concurrency).await?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        "family sources collected"
    );
    let snapshot = dataset.snapshot_for(project).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    let target = NamedNode::new(target)?;
    ensure!(
        view.text(&target, "renderer").await? == "specification",
        "family target requires the specification renderer"
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
    for processor in sources {
        let snapshot = dataset
            .snapshot_for_processor(project, processor.as_str())
            .await?;
        let source = RevisionView::new(dataset.clone(), snapshot.clone());
        let pattern = source.pattern(
            "?ontology a <http://www.w3.org/2002/07/owl#Ontology>",
            0,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?ontology WHERE {{ {pattern} }}"
            ))
            .await?;
        let Some(row) = rows.next().await? else {
            continue;
        };
        ensure!(
            rows.next().await?.is_none(),
            "one source file must declare at most one ontology"
        );
        let Term::NamedNode(ontology) = &row["ontology"] else {
            anyhow::bail!("ontology must have a named identity")
        };
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
        let pattern = source.pattern("?subject a ?kind", 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?subject WHERE {{ {pattern} \
                 FILTER(isIRI(?subject) && STRSTARTS(STR(?subject), {}) && \
                 ?subject != {ontology}) }} ORDER BY ?subject",
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
                known.len() <= 8000,
                "family exceeds 8000-resource publication budget"
            );
            let mut descriptions = Vec::new();
            for property in [
                "http://www.w3.org/2000/01/rdf-schema#comment",
                "http://purl.org/dc/terms/description",
                "http://www.w3.org/2004/02/skos/core#definition",
            ] {
                descriptions.extend(english(&source, subject, property).await?);
            }
            if descriptions.is_empty() {
                missing += 1;
            }
            terms.push(subject.clone());
        }
        let labels = english(
            &source,
            ontology,
            "http://www.w3.org/2000/01/rdf-schema#label",
        )
        .await?;
        ensure!(
            labels.len() == 1 && !labels[0].trim().is_empty(),
            "ontology {ontology} requires one English label for its chapter \
             title"
        );
        members.push(Member {
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
    let mut stream = writer
        .begin(
            project,
            owner.as_str(),
            target.as_str(),
            &format!("family-v5:{snapshot}"),
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
    for member in &members {
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
            // Keep supplied prose; indent continuation lines within the list
            // item.
            overview.push_str(&format!(
                "\n  {}\n",
                description.replace('\n', "\n  ")
            ));
        }
        overview.push('\n');
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
    for (order, member) in members.iter().enumerate() {
        component(
            &mut data,
            &target,
            &profile,
            &member.ontology,
            &member.title,
            "section",
            order as u32 + 3,
        );
        let comments = english(
            &member.view,
            &member.ontology,
            "http://www.w3.org/2000/01/rdf-schema#comment",
        )
        .await?;
        let labels = english(
            &member.view,
            &member.ontology,
            "http://www.w3.org/2000/01/rdf-schema#label",
        )
        .await?;
        let introduction = format!(
            "## Ontology metadata\n\n{}\n\nIRI: `{}`\n\nPrefix: \
             `{}`\n\nNamespace: `{}`\n\n{}\n",
            labels.join("; "),
            member.ontology.as_str(),
            member.prefix,
            member.namespace,
            comments.join("\n\n")
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
        for (position, subject) in member.terms.iter().enumerate() {
            let documentor = AnnotatedTermDocumentor::new(
                dataset.clone(),
                member.snapshot.clone(),
                project,
                subject.clone(),
                "en",
                prefixes.clone(),
                known.clone(),
            )?
            .with_source_annotations();
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
                    Literal::from(position as u32 + 1),
                ),
            ]);
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
        members.len() as u32 + 3,
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
    futures::stream::iter(jobs.into_iter().map(|(snapshot, subject)| {
        let dataset = dataset.clone();
        let prefixes = prefixes.clone();
        let known = known.clone();
        async move {
            AnnotatedTermDocumentor::new(
                dataset, snapshot, project, subject, "en", prefixes, known,
            )?
            .with_source_annotations()
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
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(project, &target, writer)
        .await?;
    Ok((plan, ontology_count, term_count))
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
