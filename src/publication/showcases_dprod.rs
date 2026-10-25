//! Native DPROD reference fixture: RDF collection, term documentation, and
//! publication assembly. No ReSpec page or JavaScript generator is executed.
use {
    super::{
        assembly::Assembler,
        collect::{Collector, MarkdownFile, RdfFile, collect_all},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        plan::{ResolvedPlan, TargetPlanner},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    std::{
        collections::{BTreeMap, HashSet},
        path::Path,
        sync::Arc,
    },
};
const PROJECT: &str = "urn:dprod-showcase:project";
const BASE: &str = "urn:dprod-showcase:";
const GROUPS: [(&str, &str, &str); 6] = [
    (
        "classes",
        "OWL classes",
        "http://www.w3.org/2002/07/owl#Class",
    ),
    (
        "object-properties",
        "Object properties",
        "http://www.w3.org/2002/07/owl#ObjectProperty",
    ),
    (
        "datatype-properties",
        "Datatype properties",
        "http://www.w3.org/2002/07/owl#DatatypeProperty",
    ),
    (
        "concept-schemes",
        "Concept schemes",
        "http://www.w3.org/2004/02/skos/core#ConceptScheme",
    ),
    (
        "node-shapes",
        "SHACL node shapes",
        "http://www.w3.org/ns/shacl#NodeShape",
    ),
    (
        "property-shapes",
        "SHACL property shapes",
        "http://www.w3.org/ns/shacl#PropertyShape",
    ),
];

pub(super) async fn collect(
    sources: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let result = build(sources, &dataset, &writer).await;
    drop(writer);
    worker.await??;
    Ok((dataset, result?))
}
async fn build(
    sources: &Path,
    dataset: &Dataset,
    writer: &GraphWriter,
) -> Result<Vec<ResolvedPlan>> {
    let authored = [
        (
            "abstract",
            "Abstract",
            "abstract",
            sources.join("abstract.md"),
        ),
        (
            "status",
            "Status of This Document",
            "status",
            sources.join("status.md"),
        ),
        (
            "overview",
            "Purpose and sources",
            "section",
            sources.join("overview.md"),
        ),
        (
            "example",
            "Dataset schema example",
            "section",
            sources.join("sources/dataset-schema.md"),
        ),
    ];
    let mut work: Vec<Arc<dyn Collector>> = Vec::new();
    let mut markdown = Vec::new();
    for (key, title, kind, path) in authored {
        let collector = Arc::new(MarkdownFile::new(
            PROJECT,
            &format!("{BASE}collector:{key}"),
            path,
        )?);
        markdown.push((key, title, kind, collector.fragment_id()));
        work.push(collector);
    }
    for (key, path) in [
        ("definitions", sources.join("project.ttl")),
        ("ontology", sources.join("sources/ontology.ttl")),
        ("shapes", sources.join("sources/shapes.ttl")),
    ] {
        work.push(Arc::new(RdfFile::new(
            PROJECT,
            &format!("{BASE}collector:{key}"),
            path,
        )?));
    }
    let website = tokio::fs::canonicalize(sources.join("website")).await?;
    for file in ["index.md", "guides/getting-started.md", "stories.md"] {
        work.push(Arc::new(MarkdownFile::in_site(
            PROJECT,
            &format!("{BASE}collector:website:{file}"),
            website.clone(),
            website.join(file),
        )?));
    }
    collect_all(work, writer, 4).await?;
    let collected = dataset.snapshot_for(PROJECT).await?;
    let view = RevisionView::new(dataset.clone(), collected.clone());
    let prefixes = Arc::new(BTreeMap::from([
        (
            "https://www.omg.org/spec/DPROD/dprod/".into(),
            "dprod".into(),
        ),
        (
            "https://www.omg.org/spec/DPROD/shapes/".into(),
            "dprod-shapes".into(),
        ),
        (
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#".into(),
            "rdf".into(),
        ),
        (
            "http://www.w3.org/2000/01/rdf-schema#".into(),
            "rdfs".into(),
        ),
        (
            "http://www.w3.org/2001/XMLSchema#".into(),
            "xsd".into(),
        ),
        (
            "http://www.w3.org/2002/07/owl#".into(),
            "owl".into(),
        ),
        (
            "http://www.w3.org/2004/02/skos/core#".into(),
            "skos".into(),
        ),
        ("http://www.w3.org/ns/shacl#".into(), "sh".into()),
        ("http://www.w3.org/ns/dcat#".into(), "dcat".into()),
        ("http://www.w3.org/ns/prov#".into(), "prov".into()),
        ("http://purl.org/dc/terms/".into(), "dct".into()),
        (
            "http://www.w3.org/ns/odrl/2/".into(),
            "odrl".into(),
        ),
    ]));
    let mut groups = Vec::new();
    let mut known = HashSet::new();
    for (key, title, class) in GROUPS {
        let pattern = view.pattern(&format!("?subject a <{class}>"), 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?subject WHERE {{ {pattern} }} ORDER BY \
                 ?subject"
            ))
            .await?;
        let mut subjects = Vec::new();
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(subject) = &row["subject"] else {
                anyhow::bail!("DPROD reference terms require named identities")
            };
            ensure!(
                known.insert(subject.clone()),
                "term appears in multiple reference categories"
            );
            subjects.push(subject.clone());
        }
        ensure!(
            !subjects.is_empty(),
            "DPROD fixture has no {title}"
        );
        groups.push((key, title, subjects));
    }
    let known = Arc::new(known);
    let mut bindings = writer
        .begin(
            PROJECT,
            "urn:dprod-showcase:bindings",
            "urn:dprod-showcase:bindings",
            &format!("dprod-reference-v1:{collected}"),
        )
        .await?;
    let mut order = 0u32;
    for (key, title, kind, fragment) in &markdown[..3] {
        place(&mut bindings, key, title, kind, order).await?;
        order += 1;
        let subject = view.node(fragment, "subject").await?;
        bindings
            .write(quad(&node("scope"), "include", subject))
            .await?;
        bindings
            .write(quad(
                &node(&format!("component:{key}")),
                "fragment",
                fragment.clone(),
            ))
            .await?;
    }
    for (key, title, subjects) in groups {
        place(&mut bindings, key, title, "section", order).await?;
        order += 1;
        for (position, subject) in subjects.into_iter().enumerate() {
            let documentor = AnnotatedTermDocumentor::new(
                dataset.clone(),
                collected.clone(),
                PROJECT,
                subject.clone(),
                "en",
                prefixes.clone(),
                known.clone(),
            )?;
            documentor.document(writer).await?;
            bindings
                .write(quad(&node("scope"), "include", subject))
                .await?;
            bindings
                .write(quad(
                    &node(&format!("component:{key}")),
                    "fragment",
                    documentor.fragment_id(),
                ))
                .await?;
            bindings
                .write(quad(
                    &documentor.fragment_id(),
                    "order",
                    Literal::from(position as u32),
                ))
                .await?;
        }
    }
    let (key, title, kind, fragment) = &markdown[3];
    place(&mut bindings, key, title, kind, order).await?;
    bindings
        .write(quad(
            &node("scope"),
            "include",
            view.node(fragment, "subject").await?,
        ))
        .await?;
    bindings
        .write(quad(
            &node(&format!("component:{key}")),
            "fragment",
            fragment.clone(),
        ))
        .await?;
    let date = view
        .optional_by(
            &NamedNode::new("https://www.omg.org/spec/DPROD/dprod/")?,
            &NamedNode::new("http://purl.org/dc/terms/issued")?,
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("DPROD source has no publication date")
        })?;
    bindings
        .write(quad(
            &node("specification"),
            "publicationDate",
            date,
        ))
        .await?;
    bindings.complete().await?;
    super::website::bind(dataset, writer, PROJECT, node("website").as_str())
        .await?;
    let revision = Assembler::new(dataset.clone()).assemble(PROJECT).await?;
    let planner = TargetPlanner::new(dataset.clone(), revision);
    let mut plans = Vec::new();
    for target in [node("specification"), node("website"), node("pdf")] {
        plans.push(planner.resolve(PROJECT, &target, writer).await?);
    }
    Ok(plans)
}
fn node(key: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{BASE}{key}"))
}
fn quad(subject: &NamedNode, field: &str, object: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        term(field),
        object,
        GraphName::DefaultGraph,
    )
}
async fn place(
    stream: &mut super::writer::ContributionStream,
    key: &str,
    title: &str,
    kind: &str,
    order: u32,
) -> Result<()> {
    let placement = node(&format!("placement:{key}"));
    let component = node(&format!("component:{key}"));
    let metadata: [(NamedNode, NamedNode, Term); 7] = [
        (
            node("profile"),
            term("placement"),
            placement.clone().into(),
        ),
        (
            placement.clone(),
            NamedNode::new_unchecked(super::RDF_TYPE),
            term("Placement").into(),
        ),
        (
            placement.clone(),
            term("component"),
            component.clone().into(),
        ),
        (
            placement.clone(),
            term("order"),
            Literal::from(order).into(),
        ),
        (
            placement,
            term("sectionKind"),
            Literal::new_simple_literal(kind).into(),
        ),
        (
            component.clone(),
            NamedNode::new_unchecked(super::RDF_TYPE),
            term("Component").into(),
        ),
        (
            component,
            NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
            Literal::new_simple_literal(title).into(),
        ),
    ];
    // A PDF has its own placement records, even when it shares reference
    // content.
    let pdf_placement = node(&format!("pdf-placement:{key}"));
    let pdf_metadata: [(NamedNode, &str, Term); 4] = [
        (
            node("pdf-profile"),
            "placement",
            pdf_placement.clone().into(),
        ),
        (
            pdf_placement.clone(),
            "component",
            node(&format!("component:{key}")).into(),
        ),
        (
            pdf_placement.clone(),
            "order",
            Literal::from(order).into(),
        ),
        (
            pdf_placement.clone(),
            "sectionKind",
            Literal::new_simple_literal(
                if key == "example" { "appendix" } else { kind },
            )
            .into(),
        ),
    ];
    for (subject, field, object) in pdf_metadata {
        stream.write(quad(&subject, field, object)).await?;
    }
    stream
        .write(Quad::new(
            pdf_placement,
            NamedNode::new_unchecked(super::RDF_TYPE),
            term("Placement"),
            GraphName::DefaultGraph,
        ))
        .await?;
    for (subject, predicate, object) in metadata {
        stream
            .write(Quad::new(
                subject,
                predicate,
                object,
                GraphName::DefaultGraph,
            ))
            .await?;
    }
    Ok(())
}
