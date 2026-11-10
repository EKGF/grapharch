use {
    super::{discover::Inventory, quad, title_quad},
    crate::publication::{
        collect::Collector,
        dataset::{Dataset, Stage, identity},
        document::{AnnotatedTermDocumentor, Documentor},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Term},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

pub(super) async fn title(
    dataset: &Dataset,
    view: &RevisionView,
    inventory: &Inventory,
) -> Result<String> {
    let mut roots = dataset
        .select(format!(
            "SELECT DISTINCT ?s WHERE {{ {} }} ORDER BY ?s LIMIT 2",
            view.pattern("?s a <http://www.w3.org/2002/07/owl#Ontology>", 0)
        ))
        .await?;
    if let Some(row) = roots.next().await? &&
        roots.next().await?.is_none() &&
        let Term::NamedNode(subject) = &row["s"] &&
        let Some(label) = label(view, subject).await?
    {
        return Ok(label);
    }
    let mut pages = dataset
        .select(format!(
            "SELECT ?title WHERE {{ {} }} LIMIT 1",
            view.pattern(
                &format!(
                    "?page a {} ; {} \"\" ; <http://purl.org/dc/terms/title> \
                     ?title",
                    term("Page"),
                    term("route")
                ),
                0
            )
        ))
        .await?;
    if let Some(row) = pages.next().await? &&
        let Term::Literal(title) = &row["title"]
    {
        return Ok(title.value().to_owned());
    }
    Ok(inventory
        .root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Documentation")
        .replace(['-', '_'], " "))
}

async fn label(
    view: &RevisionView,
    subject: &NamedNode,
) -> Result<Option<String>> {
    for predicate in [
        "http://purl.org/dc/terms/title",
        "http://www.w3.org/2004/02/skos/core#prefLabel",
        "http://www.w3.org/2000/01/rdf-schema#label",
        "http://www.w3.org/ns/shacl#name",
    ] {
        let mut rows = view
            .objects_by(subject, &NamedNode::new_unchecked(predicate))
            .await?;
        while let Some(row) = rows.next().await? {
            if let Term::Literal(value) = &row["value"] &&
                value.language().is_none_or(|language| {
                    language.eq_ignore_ascii_case("en")
                }) &&
                !value.value().trim().is_empty()
            {
                return Ok(Some(value.value().to_owned()));
            }
        }
    }
    Ok(None)
}

pub(super) async fn defaults(
    writer: &GraphWriter,
    project: &NamedNode,
    target: &NamedNode,
    title: &str,
) -> Result<()> {
    let scope = identity("quick-scope", &[project.as_str()]);
    let profile = identity("quick-profile", &[project.as_str()]);
    let mut stream = writer
        .begin_stage(
            project.as_str(),
            "urn:grapharch:quick-defaults",
            project.as_str(),
            "quick-defaults-v1",
            Stage::Document,
        )
        .await?;
    for q in [
        quad(project, "type", term("Project")),
        title_quad(project, title),
        quad(project, "target", target.clone()),
        quad(target, "type", term("Target")),
        title_quad(target, title),
        quad(target, "scope", scope.clone()),
        quad(target, "profile", profile.clone()),
        quad(
            target,
            "renderer",
            crate::publication::Renderer::Website.iri(),
        ),
        quad(
            target,
            "destination",
            Literal::new_simple_literal("site"),
        ),
        quad(&scope, "type", term("Scope")),
        quad(&profile, "type", term("Profile")),
    ] {
        stream.write(q).await?;
    }
    stream.complete().await
}

pub(super) async fn rdf_pages(
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &NamedNode,
    inventory: &Inventory,
) -> Result<()> {
    let snapshot = dataset.snapshot_collected_for(project.as_str()).await?;
    let view = RevisionView::new(dataset.clone(), snapshot.clone());
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?s WHERE {{ {} FILTER(isIRI(?s) && \
             !STRSTARTS(STR(?s), \"urn:grapharch:\")) }} ORDER BY ?s LIMIT \
             20001",
            view.pattern("?s a ?kind", 0)
        ))
        .await?;
    let mut known = HashSet::new(); // bounded: 20,000 named RDF subjects.
    while let Some(row) = rows.next().await? {
        ensure!(
            known.len() < 20_000,
            "quick reference exceeds 20,000 subjects; configure a scoped \
             publication"
        );
        if let Term::NamedNode(subject) = &row["s"] {
            known.insert(subject.clone());
        }
    }
    let known = Arc::new(known);
    let prefixes = Arc::new(BTreeMap::from([
        (
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#".into(),
            "rdf".into(),
        ),
        (
            "http://www.w3.org/2000/01/rdf-schema#".into(),
            "rdfs".into(),
        ),
        (
            "http://www.w3.org/2002/07/owl#".into(),
            "owl".into(),
        ),
        (
            "http://www.w3.org/2001/XMLSchema#".into(),
            "xsd".into(),
        ),
        ("http://www.w3.org/ns/shacl#".into(), "sh".into()),
        (
            "http://www.w3.org/2004/02/skos/core#".into(),
            "skos".into(),
        ),
    ]));
    let diagram_renderer = Arc::new(
        crate::publication::document::plantuml::LocalRenderer::new(
            std::ffi::OsStr::new("plantuml"),
        ),
    );
    let mut has_home = has_home(dataset, &view).await?;
    let mut any = false;
    let mut documented: BTreeMap<NamedNode, String> = BTreeMap::new(); // bounded: shared 20,000 subject budget.
    for path in inventory
        .files
        .iter()
        .filter(|path| super::discover::rdf(path))
    {
        let relative = crate::publication::collect::page::source_path(
            &inventory.root,
            path,
        )?;
        let collector =
            identity("quick-source", &[project.as_str(), &relative]);
        let source_snapshot = dataset
            .snapshot_for_processor(project.as_str(), collector.as_str())
            .await?;
        let source_view =
            RevisionView::new(dataset.clone(), source_snapshot.clone());
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?s WHERE {{ {} FILTER(isIRI(?s) && \
                 !STRSTARTS(STR(?s), \"urn:grapharch:\")) }} ORDER BY ?s",
                source_view.pattern("?s a ?kind", 0)
            ))
            .await?;
        let mut terms = Vec::new(); // bounded: shared 20,000 subject budget.
        let mut aliases = std::collections::BTreeSet::new(); // bounded: 256 source pages.
        while let Some(row) = rows.next().await? {
            if let Term::NamedNode(subject) = &row["s"] {
                ensure!(
                    terms.len() < 20_000,
                    "reference exceeds subject budget"
                );
                if let Some(owner) = documented.get(subject) {
                    aliases.insert(owner.clone());
                } else {
                    documented.insert(subject.clone(), relative.clone());
                    terms.push(subject.clone());
                }
            }
        }
        if terms.is_empty() {
            if !aliases.is_empty() {
                super::aliases::collect(writer, project, &relative, aliases)
                    .await?;
            }
            continue;
        }
        any = true;
        let page = identity("quick-rdf-page", &[project.as_str(), &relative]);
        let title = if let Some(root) = terms.first() {
            label(&source_view, root).await?
        } else {
            None
        }
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap()
                .to_string_lossy()
                .replace(['-', '_'], " ")
        });
        let route = if has_home {
            format!(
                "reference/{}/",
                crate::publication::collect::page::slug(&relative)
            )
        } else {
            has_home = true;
            String::new()
        };
        let mut stream = writer
            .begin_stage(
                project.as_str(),
                "urn:grapharch:quick-rdf-pages",
                &relative,
                snapshot.as_str(),
                Stage::Document,
            )
            .await?;
        for q in [
            quad(&page, "type", term("Page")),
            title_quad(&page, &title),
            quad(
                &page,
                "sourcePath",
                Literal::new_simple_literal(&relative),
            ),
            quad(&page, "route", Literal::new_simple_literal(route)),
            quad(&page, "navigationHidden", Literal::from(false)),
            quad(&page, "navigationOrder", Literal::from(500)),
        ] {
            stream.write(q).await?;
        }
        for (order, subject) in terms.into_iter().enumerate() {
            let documentor = AnnotatedTermDocumentor::new(
                dataset.clone(),
                snapshot.clone(),
                project.as_str(),
                subject.clone(),
                "en",
                prefixes.clone(),
                known.clone(),
            )?
            .with_diagram_renderer(diagram_renderer.clone())
            .with_source_annotations();
            documentor.document(writer).await?;
            let fragment = documentor.fragment_id();
            let topic = identity("quick-rdf-topic", &[
                page.as_str(),
                subject.as_str(),
            ]);
            for q in [
                quad(&page, "topic", topic.clone()),
                quad(&topic, "fragment", fragment.clone()),
                quad(&fragment, "partOf", page.clone()),
                quad(&fragment, "order", Literal::from(order as u32)),
                quad(&topic, "type", term("Topic")),
            ] {
                stream.write(q).await?;
            }
            // Scope follows the real documented subject, not a synthetic class
            // assertion.
            stream
                .write(quad(
                    &identity("quick-scope", &[project.as_str()]),
                    "include",
                    subject,
                ))
                .await?;
        }
        stream.complete().await?;
    }
    ensure!(
        any || inventory
            .files
            .iter()
            .any(|path| path.extension().is_some_and(|ext| ext == "md")),
        "no named typed RDF resources or Markdown pages found; declare \
         ontology terms or provide a Markdown introduction"
    );
    Ok(())
}

async fn has_home(dataset: &Dataset, view: &RevisionView) -> Result<bool> {
    let mut rows = dataset
        .select(format!(
            "SELECT ?page WHERE {{ {} }} LIMIT 1",
            view.pattern(
                &format!(
                    "?page a {} ; {} \"\"",
                    term("Page"),
                    term("route")
                ),
                0
            )
        ))
        .await?;
    Ok(rows.next().await?.is_some())
}

pub(super) async fn home(
    dataset: &Dataset,
    writer: &GraphWriter,
    project: &NamedNode,
    title: &str,
) -> Result<()> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(project.as_str()).await?,
    );
    if has_home(dataset, &view).await? {
        return Ok(());
    }
    let mut rows = dataset
        .select(format!(
            "SELECT ?path ?title WHERE {{ {} }} ORDER BY ?path",
            view.pattern(
                &format!(
                    "?page a {} ; {} ?path ; <http://purl.org/dc/terms/title> \
                     ?title",
                    term("Page"),
                    term("sourcePath")
                ),
                0
            )
        ))
        .await?;
    let mut body = format!("# {title}\n\n"); // bounded: discovery limits pages to 256.
    while let Some(row) = rows.next().await? {
        if let (Term::Literal(path), Term::Literal(title)) =
            (&row["path"], &row["title"])
        {
            body.push_str(&format!(
                "- [{}]({})\n",
                title.value().replace(['[', ']'], ""),
                path.value()
            ));
        }
    }
    let id = identity("quick-home", &[project.as_str()]);
    crate::publication::collect::MarkdownFile::generated(
        project.as_str(),
        "urn:grapharch:quick-home",
        crate::publication::collect::GeneratedPage {
            source: project.as_str(),
            id: &id,
            path: "index.md",
            title,
            route: "",
            body,
            revision: "quick-home-v1".into(),
        },
    )?
    .collect(writer)
    .await
}
