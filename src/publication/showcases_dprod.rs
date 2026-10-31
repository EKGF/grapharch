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
mod source_owned;

enum Authored {
    Fixture(Vec<(String, String, String, NamedNode)>),
    Configured(Vec<NamedNode>),
}

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
        markdown.push((
            key.to_owned(),
            title.to_owned(),
            kind.to_owned(),
            collector.fragment_id(),
        ));
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
    assemble(
        dataset,
        writer,
        Authored::Fixture(markdown),
        "https://www.omg.org/spec/DPROD/dprod/",
        "https://www.omg.org/spec/DPROD/shapes/",
        false,
    )
    .await
}
async fn assemble(
    dataset: &Dataset,
    writer: &GraphWriter,
    authored: Authored,
    ontology: &str,
    shapes: &str,
    derive: bool,
) -> Result<Vec<ResolvedPlan>> {
    let collected = dataset.snapshot_for(PROJECT).await?;
    let view = RevisionView::new(dataset.clone(), collected.clone());
    let roots = if matches!(authored, Authored::Configured(_)) {
        source_owned::roots(&view).await?
    } else {
        Vec::new()
    };
    let prefixes = Arc::new(BTreeMap::from([
        (ontology.into(), "dprod".into()),
        (shapes.into(), "dprod-shapes".into()),
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
            if known.insert(subject.clone()) {
                ensure!(
                    known.len() <= 16384,
                    "DPROD reference resource budget exceeded"
                );
                subjects.push(subject.clone());
            }
        }
        if !subjects.is_empty() {
            groups.push((key, title, subjects));
        }
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
    if derive {
        let native = NamedNode::new(ontology)?;
        for (target, predicate, native_predicate) in [
            (
                node("project"),
                NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                "http://purl.org/dc/terms/title",
            ),
            (
                node("specification"),
                NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                "http://purl.org/dc/terms/title",
            ),
            (
                node("specification"),
                NamedNode::new_unchecked(
                    "http://purl.org/dc/terms/description",
                ),
                "http://purl.org/dc/terms/description",
            ),
            (
                node("specification"),
                term("license"),
                "http://purl.org/dc/terms/license",
            ),
        ] {
            ensure!(
                view.optional_by(&target, &predicate).await?.is_none(),
                "derived native header conflicts with source settings"
            );
            let value = view
                .optional_by(&native, &NamedNode::new(native_predicate)?)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "native DPROD ontology missing {native_predicate}"
                    )
                })?;
            bindings
                .write(Quad::new(
                    target,
                    predicate,
                    value,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
    }
    let mut assets = dataset
        .select(format!(
            "SELECT DISTINCT ?asset WHERE {{ {} }}",
            view.pattern(&format!("?fragment {} ?asset", term("asset")), 0)
        ))
        .await?;
    while let Some(row) = assets.next().await? {
        let Term::NamedNode(asset) = &row["asset"] else {
            anyhow::bail!("image asset must be IRI")
        };
        for scope in [node("scope"), node("website-scope")] {
            bindings
                .write(quad(&scope, "include", asset.clone()))
                .await?;
        }
    }
    let mut order = 0u32;
    if let Authored::Configured(fragments) = &authored {
        for fragment in fragments {
            bindings
                .write(quad(
                    &node("scope"),
                    "include",
                    view.node(fragment, "subject").await?,
                ))
                .await?;
        }
    }
    if let Authored::Fixture(markdown) = &authored {
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
    }
    for (key, title, subjects) in groups {
        if roots.is_empty() {
            place(&mut bindings, key, title, "section", order).await?;
        } else {
            for root in &roots {
                root.place(&mut bindings, key, title, order + 1).await?;
            }
        }
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
    if let Authored::Fixture(markdown) = &authored {
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
    }
    let date = view
        .optional_by(
            &NamedNode::new(ontology)?,
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

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteConfig {
    repository:  String,
    r#ref:       String,
    definitions: String,
}
pub(super) async fn collect_remote(
    config: &Path,
    cache: &Path,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    use tokio::io::AsyncReadExt;
    let input = tokio::fs::File::open(config).await?;
    let mut bytes = Vec::new();
    input.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
    ensure!(
        bytes.len() <= 64 * 1024,
        "DPROD transport settings exceed 64 KiB budget"
    );
    let config: RemoteConfig = serde_yaml::from_slice(&bytes)?;
    ensure!(
        config.r#ref == "develop",
        "DPROD showcase must use develop"
    );
    let scan = super::collect::git::remote_repository(
        &config.repository,
        &config.r#ref,
        cache,
        PROJECT,
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let result = async {
        scan.provenance.collect(&writer).await?;
        scan.blob(Path::new(&config.definitions))
            .await?
            .rdf_collector("urn:dprod-showcase:settings")?
            .collect(&writer)
            .await?;
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for(PROJECT).await?,
        );
        let mut definition_rows =
            view.objects(&node("project"), "collector").await?;
        let mut definitions = Vec::new();
        while let Some(row) = definition_rows.next().await? {
            ensure!(
                definitions.len() < 64,
                "DPROD exceeds collector budget"
            );
            definitions.push(row["value"].clone());
        }
        definitions.sort_by_key(ToString::to_string);
        let mut work = Vec::<Arc<dyn Collector>>::new();
        let mut fragments = HashSet::new();
        let mut subjects = BTreeMap::new();
        let mut images = HashSet::new();
        for definition in definitions {
            let Term::NamedNode(definition) = definition else {
                anyhow::bail!("collector must be IRI")
            };
            let kind = view.text(&definition, "kind").await?;
            let path = view.text(&definition, "sourcePath").await?;
            let blob = scan.blob(Path::new(&path)).await?;
            match kind.as_str() {
                "code" => {
                    let fragment = view.node(&definition, "fragment").await?;
                    ensure!(
                        fragments.insert(fragment),
                        "duplicate authored fragment"
                    );
                    work.push(Arc::new(super::collect::git::GitCodeFile::new(
                        PROJECT,
                        definition.as_str(),
                        blob,
                        view.node(&definition, "fragment").await?,
                        view.node(&definition, "subject").await?,
                        &view.text(&definition, "format").await?,
                    )?));
                },
                "rdf" => {
                    subjects
                        .insert(path, view.node(&definition, "subject").await?);
                    work.push(blob.rdf_collector(definition.as_str())?);
                },
                "markdown" | "markdown-page" => {
                    let fragment = view.node(&definition, "fragment").await?;
                    let mut collector =
                        super::collect::git::GitMarkdownFile::new(
                            blob,
                            definition.as_str(),
                            fragment.clone(),
                            kind == "markdown-page",
                        )?;
                    if kind == "markdown-page" {
                        let title = view
                            .optional_by(
                                &definition,
                                &NamedNode::new_unchecked(
                                    "http://purl.org/dc/terms/title",
                                ),
                            )
                            .await?
                            .ok_or_else(|| {
                                anyhow::anyhow!("page hint title missing")
                            })?;
                        let Term::Literal(title) = title else {
                            anyhow::bail!("page title must be literal")
                        };
                        collector = collector.with_page_hints(
                            title.value().into(),
                            view.text(&definition, "route").await?,
                        )?;
                    }
                    for image in collector.image_collectors().await? {
                        if images.insert(image.identity()?.source.to_owned()) {
                            ensure!(
                                images.len() <= 256,
                                "DPROD exceeds image asset budget"
                            );
                            work.push(image);
                        }
                    }
                    ensure!(
                        fragments.insert(fragment),
                        "duplicate authored fragment"
                    );
                    work.push(Arc::new(collector));
                },
                _ => anyhow::bail!("unsupported DPROD collector kind {kind}"),
            }
        }
        collect_all(work, &writer, 4).await?;
        super::collect::HtmlPreambleFile::from_git(
            PROJECT,
            "urn:dprod-showcase:collector:preamble",
            &scan.blob(Path::new("respec/template.html")).await?,
            &["urn:dprod-showcase:specification", "urn:dprod-showcase:pdf"],
        )?
        .collect(&writer)
        .await?;
        let ontology = subjects
            .get("ontology/dprod/dprod-ontology.ttl")
            .ok_or_else(|| anyhow::anyhow!("ontology hint missing"))?;
        let shapes = subjects
            .get("ontology/dprod/dprod-shapes.ttl")
            .ok_or_else(|| anyhow::anyhow!("shapes hint missing"))?;
        assemble(
            &dataset,
            &writer,
            Authored::Configured(fragments.into_iter().collect()),
            ontology.as_str(),
            shapes.as_str(),
            true,
        )
        .await
    }
    .await;
    drop(writer);
    worker.await??;
    Ok((dataset, result?))
}
