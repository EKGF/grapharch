//! Generate ontology reference publications and retain their artifacts in the
//! companion website KG. Sources are read only during collection/documentation.
use {
    super::{
        assembly::Assembler,
        collect::{
            Collector,
            RdfFile,
            collect_all,
            published_artifact::PublishedArtifactFile,
        },
        dataset::{Dataset, identity},
        document::{AnnotatedTermDocumentor, Documentor},
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Term},
    std::{collections::HashSet, path::Path, sync::Arc},
};

mod dependencies;
mod structure;
use structure::{placement, q};

pub(crate) async fn collect(
    root: &Path,
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
) -> Result<()> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(project).await?,
    );
    let mut targets = view.objects(&NamedNode::new(project)?, "target").await?;
    let mut configured = Vec::new();
    while let Some(row) = targets.next().await? {
        let Term::NamedNode(target) = &row["value"] else {
            anyhow::bail!("publication target must be an IRI")
        };
        if let Some(source) = view.optional(target, "ontologySource").await? {
            let Term::Literal(source) = source else {
                anyhow::bail!("ontologySource must be a path literal")
            };
            ensure!(
                configured.len() < 4,
                "website exceeds four ontology specification targets"
            );
            configured.push((target.clone(), source.value().to_owned()));
        }
    }
    for (target, source) in configured {
        build(root, dataset, writer, project, &target, &source).await?;
    }
    Ok(())
}

async fn build(
    root: &Path,
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &str,
    target: &NamedNode,
    source: &str,
) -> Result<()> {
    let path = tokio::fs::canonicalize(root.join(source)).await?;
    ensure!(
        path.starts_with(root),
        "ontologySource escapes documentation root"
    );
    let collector = identity("ontology-source", &[project, target.as_str()]);
    RdfFile::new(project, collector.as_str(), path)?
        .collect(writer)
        .await?;
    let snapshot = dataset.snapshot_for(project).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    ensure!(
        view.renderer(target).await?.as_str() == "specification",
        "ontologySource requires a specification target"
    );
    let scope = view.node(target, "scope").await?;
    let profile = view.node(target, "profile").await?;
    let source_snapshot = dataset
        .snapshot_for_processor(project, collector.as_str())
        .await?;
    let source_view =
        RevisionView::new(dataset.clone(), source_snapshot.clone());
    let pattern = source_view.pattern(
        "?ontology a <http://www.w3.org/2002/07/owl#Ontology>",
        0,
    );
    let mut ontologies = dataset
        .select(format!(
            "SELECT DISTINCT ?ontology WHERE {{ {pattern} }}"
        ))
        .await?;
    let row = ontologies.next().await?.ok_or_else(|| {
        anyhow::anyhow!("ontology source has no owl:Ontology declaration")
    })?;
    let Term::NamedNode(ontology) = &row["ontology"] else {
        anyhow::bail!("ontology requires a named identity")
    };
    ensure!(
        ontologies.next().await?.is_none(),
        "ontology specification requires one ontology declaration"
    );
    let mut namespace_values = Vec::new();
    for property in [
        "http://purl.org/vocab/vann/preferredNamespaceUri",
        "http://purl.org/vocab/vann/preferredNamespacePrefix",
    ] {
        let value = source_view
            .optional_by(ontology, &NamedNode::new(property)?)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("ontology must declare {property}")
            })?;
        let Term::Literal(value) = value else {
            anyhow::bail!("preferred namespace declarations must be literals")
        };
        namespace_values.push(value.value().to_owned());
    }
    let prefixes = structure::prefixes(&namespace_values);
    let mut groups = Vec::new();
    let mut known = HashSet::new(); // bounded: coordinated resource and identity-byte budgets.
    let mut identity_bytes = 0usize;
    for (kind, title) in [
        ("Class", "Classes"),
        ("ObjectProperty", "Object properties"),
        ("DatatypeProperty", "Datatype properties"),
    ] {
        let pattern = source_view.pattern(&format!("?subject a <http://www.w3.org/2002/07/owl#{kind}> ; <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> {ontology}"), 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?subject WHERE {{ {pattern} \
                 FILTER(isIRI(?subject)) }} ORDER BY ?subject"
            ))
            .await?;
        let mut terms = Vec::new();
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(subject) = &row["subject"] else {
                anyhow::bail!("ontology terms require named identities")
            };
            ensure!(
                known.insert(subject.clone()),
                "ontology term has multiple incompatible kinds"
            );
            identity_bytes += subject.as_str().len();
            ensure!(
                known.len() <= super::limits::FAMILY_RESOURCES &&
                    identity_bytes <= super::limits::REFERENCE_INDEX_BYTES,
                "ontology specification exceeds coordinated resource or \
                 identity-byte budget"
            );
            terms.push(subject.clone());
        }
        groups.push((kind, title, terms));
    }
    ensure!(
        !known.is_empty(),
        "ontology specification has no defined terms"
    );
    let collected_snapshot = dataset.snapshot_collected_for(project).await?;
    let collected_view =
        RevisionView::new(dataset.clone(), collected_snapshot.clone());
    let external =
        dependencies::selected(dataset, &collected_view, &scope, &known)
            .await?;
    let attribution =
        dependencies::sources(dataset, &collected_view, &external).await?;
    let external_set: HashSet<_> = external.iter().cloned().collect();
    known.extend(external.iter().cloned());
    if !external.is_empty() {
        groups.push(("ReusedVocabulary", "Reused vocabulary", external));
    }
    let known = Arc::new(known);
    let owner = identity("ontology-bindings", &[project, target.as_str()]);
    let mut stream = writer
        .begin(
            project,
            owner.as_str(),
            target.as_str(),
            &format!("ontology-spec-v2:{snapshot}"),
        )
        .await?;
    let contribution = identity("contribution", &[
        project,
        owner.as_str(),
        target.as_str(),
    ]);
    let mut write = vec![
        q(&owner, "type", term("Processor")),
        q(&contribution, "type", term("Contribution")),
        q(&contribution, "project", NamedNode::new(project)?),
        q(&contribution, "processor", owner.clone()),
        q(
            &contribution,
            "source",
            Literal::new_simple_literal(target.as_str()),
        ),
        q(
            &contribution,
            "revision",
            Literal::new_simple_literal(format!("ontology-spec-v2:{snapshot}")),
        ),
    ];
    for (position, (kind, title, property)) in [
        (
            "abstract",
            "Abstract",
            "http://purl.org/dc/terms/description",
        ),
        (
            "status",
            "Status of This Document",
            "http://www.w3.org/2004/02/skos/core#scopeNote",
        ),
        (
            "section",
            "Model and conformance",
            "http://www.w3.org/2000/01/rdf-schema#comment",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let rich = if property == "http://www.w3.org/2000/01/rdf-schema#comment"
        {
            super::document::description::read(&source_view, ontology, "en")
                .await?
        } else {
            None
        };
        let value = if let Some(body) = rich {
            Term::Literal(Literal::new_simple_literal(body))
        } else {
            source_view
                .optional_by(ontology, &NamedNode::new(property)?)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "ontology missing front-matter annotation {property}"
                    )
                })?
        };
        let Term::Literal(body) = value else {
            anyhow::bail!("ontology front matter requires literals")
        };
        let subject =
            identity("ontology-introduction", &[target.as_str(), title]);
        let fragment = identity("ontology-introduction-fragment", &[
            subject.as_str()
        ]);
        placement(
            &mut write,
            target,
            &profile,
            &subject,
            title,
            kind,
            position as u32,
        );
        write.extend([
            q(&scope, "include", subject.clone()),
            q(&subject, "fragment", fragment.clone()),
            q(&fragment, "type", term("Fragment")),
            q(&fragment, "subject", subject),
            q(&fragment, "body", body),
            q(
                &fragment,
                "format",
                Literal::new_simple_literal("text/markdown"),
            ),
            q(&fragment, "contribution", contribution.clone()),
        ]);
    }
    let diagram_renderer = Arc::new(
        crate::publication::document::plantuml::LocalRenderer::new(
            std::ffi::OsStr::new("plantuml"),
        ),
    );
    for (position, (kind, title, terms)) in groups.into_iter().enumerate() {
        let component = identity("ontology-group", &[target.as_str(), kind]);
        placement(
            &mut write,
            target,
            &profile,
            &component,
            title,
            "section",
            position as u32 + 3,
        );
        for (order, subject) in terms.into_iter().enumerate() {
            let documentor = AnnotatedTermDocumentor::new(
                dataset.clone(),
                if external_set.contains(&subject) {
                    collected_snapshot.clone()
                } else {
                    source_snapshot.clone()
                },
                project,
                subject.clone(),
                "en",
                prefixes.clone(),
                known.clone(),
            )?
            .with_diagram_renderer(diagram_renderer.clone());
            let documentor = if external_set.contains(&subject) {
                documentor.with_source_annotations()
            } else {
                documentor.with_ontology_context(ontology.clone())
            };
            documentor.document(writer).await?;
            if let Some(sources) = attribution.get(&subject) {
                write.extend(dependencies::attribution(
                    target,
                    &component,
                    &contribution,
                    &subject,
                    sources,
                    order as u32 * 2 + 1,
                ));
            }
            write.extend([
                q(&scope, "include", subject),
                q(&component, "fragment", documentor.fragment_id()),
                q(
                    &documentor.fragment_id(),
                    "order",
                    Literal::from(order as u32 * 2),
                ),
            ]);
        }
    }
    for quad in write {
        stream.write(quad).await?;
    }
    stream.complete().await?;
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(project, target, writer)
        .await?;
    let stage = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) =
        Publisher::new(dataset.clone())
            .render(&plan, stage.path())
            .await?
    else {
        anyhow::bail!("ontology specification publication was superseded")
    };
    let destination = view.text(target, "destination").await?;
    let mut work: Vec<Arc<dyn Collector>> = Vec::new();
    for file in publication.manifest.files {
        work.push(Arc::new(PublishedArtifactFile::new(
            project,
            publication.directory.join(&file),
            format!("{destination}/{file}"),
        )?));
    }
    let archive = stage.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    work.push(Arc::new(PublishedArtifactFile::new(
        project,
        archive,
        format!("{destination}/documentation.nq"),
    )?));
    collect_all(work, writer, 4).await?;
    Ok(())
}
