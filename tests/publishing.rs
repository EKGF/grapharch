use {
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        dataset::Dataset,
        plan::{ResolvedPlan, TargetPlanner},
        render::{PublishOutcome, Publisher},
        writer::GraphWriter,
    },
    oxrdf::{Literal, NamedNode},
    std::path::Path,
};

async fn plans(
    body: &str,
    source: &Path,
) -> anyhow::Result<(Dataset, ResolvedPlan, ResolvedPlan)> {
    plans_in(Dataset::memory()?, body, source).await
}

async fn plans_in(
    dataset: Dataset,
    body: &str,
    source: &Path,
) -> anyhow::Result<(Dataset, ResolvedPlan, ResolvedPlan)> {
    let source_text = include_str!("fixtures/publication/projects.ttl")
        .replace(
            "\"Shared introduction.\"",
            &Literal::new_simple_literal(body).to_string(),
        );
    let titles = "\n@prefix dct: <http://purl.org/dc/terms/> . \
                  ex:ontology-project dct:title \"Documentation graph\" . \
                  ex:spec dct:title \"Specification\" . ex:pdf dct:title \
                  \"Review document\" . ex:introduction dct:title \
                  \"Overview\" .";
    tokio::fs::write(source, format!("{source_text}{titles}")).await?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    let project = "urn:example:ontology-project";
    RdfFile::new(
        project,
        "urn:collector:definitions",
        source.to_owned(),
    )?
    .collect(&writer)
    .await?;
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    let planner = TargetPlanner::new(dataset.clone(), revision);
    let html = planner
        .resolve(
            project,
            &NamedNode::new("urn:example:spec")?,
            &writer,
        )
        .await?;
    let pdf = planner
        .resolve(
            project,
            &NamedNode::new("urn:example:pdf")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    Ok((dataset, html, pdf))
}

fn directory(outcome: PublishOutcome) -> anyhow::Result<std::path::PathBuf> {
    match outcome {
        PublishOutcome::Published(publication) => Ok(publication.directory),
        PublishOutcome::Superseded => {
            anyhow::bail!("unexpected superseded publication")
        },
    }
}

#[tokio::test]
async fn html_and_typst_pdf_publish_the_same_pinned_content_without_original_sources()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("project.ttl");
    let (dataset, html, pdf) = plans("Shared prose with **emphasis**, `code`, and a [reference](https://example.org/spec).\n\n| Term | Meaning |\n| --- | --- |\n| Data product | A governed collection |\n", &source).await?;
    tokio::fs::remove_file(&source).await?;
    let publisher = Publisher::new(dataset);
    let html_dir = directory(publisher.render(&html, temp.path()).await?)?;
    let page = tokio::fs::read_to_string(html_dir.join("index.html")).await?;
    assert!(
        page.contains("Shared prose") &&
            page.contains("<strong>emphasis</strong>") &&
            page.contains("<table>")
    );
    assert!(!page.contains("respec-w3c.js"));
    let pdf_dir = directory(publisher.render(&pdf, temp.path()).await?)?;
    assert!(
        tokio::fs::read(pdf_dir.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    assert!(
        tokio::fs::read_to_string(pdf_dir.join("main.typ"))
            .await?
            .contains("Shared prose")
    );
    let manifest =
        tokio::fs::read_to_string(pdf_dir.join("manifest.yaml")).await?;
    assert!(
        manifest.contains("input_revision:") &&
            manifest.contains("profile:") &&
            manifest.contains("scope:")
    );
    assert!(
        tokio::fs::metadata(html_dir.join("index.html"))
            .await?
            .is_file()
    );
    Ok(())
}

#[tokio::test]
async fn unowned_output_directories_are_not_replaced() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let (dataset, html, _) =
        plans("Initial prose.", &temp.path().join("project.ttl")).await?;
    let destination = temp.path().join("build/spec");
    tokio::fs::create_dir_all(&destination).await?;
    tokio::fs::write(destination.join("user.txt"), "Keep this file.").await?;
    assert!(
        Publisher::new(dataset)
            .render(&html, temp.path())
            .await
            .is_err()
    );
    assert_eq!(
        tokio::fs::read_to_string(destination.join("user.txt")).await?,
        "Keep this file."
    );
    Ok(())
}

#[tokio::test]
async fn unsupported_markdown_does_not_replace_the_last_successful_output()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let (dataset, good, _) =
        plans("Valid prose.", &temp.path().join("good.ttl")).await?;
    let target =
        directory(Publisher::new(dataset).render(&good, temp.path()).await?)?;
    let old_page = tokio::fs::read(target.join("index.html")).await?;
    let (dataset, bad, _) = plans(
        "<script>throw new Error('unsupported')</script>",
        &temp.path().join("bad.ttl"),
    )
    .await?;
    assert!(
        Publisher::new(dataset)
            .render(&bad, temp.path())
            .await
            .is_err()
    );
    assert_eq!(
        old_page,
        tokio::fs::read(target.join("index.html")).await?
    );
    Ok(())
}

#[tokio::test]
async fn later_requests_win_even_when_render_futures_are_polled_in_reverse_order()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("project.ttl");
    let (dataset, old, _) = plans("Old prose.", &source).await?;
    let (dataset, new, _) = plans_in(dataset, "New prose.", &source).await?;
    let publisher = Publisher::new(dataset);
    let target = directory(publisher.render(&old, temp.path()).await?)?;
    let old_page = tokio::fs::read_to_string(target.join("index.html")).await?;
    let old_future = publisher.render(&old, temp.path());
    let new_future = publisher.render(&new, temp.path());
    directory(new_future.await?)?;
    assert!(matches!(
        old_future.await?,
        PublishOutcome::Superseded
    ));
    let new_page = tokio::fs::read_to_string(target.join("index.html")).await?;
    assert!(
        new_page.contains("New prose.") && !new_page.contains("Old prose.")
    );
    let anchor = |page: &str| {
        page.split("class=\"fragment\" id=\"")
            .nth(1)
            .and_then(|part| part.split('"').next())
            .map(str::to_owned)
    };
    assert!(anchor(&old_page).is_some());
    assert_eq!(anchor(&old_page), anchor(&new_page));
    Ok(())
}

#[tokio::test]
async fn extra_unowned_files_inside_a_publication_are_preserved()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let (dataset, plan, _) = plans(
        "Generated prose.",
        &temp.path().join("project.ttl"),
    )
    .await?;
    let publisher = Publisher::new(dataset);
    let target = directory(publisher.render(&plan, temp.path()).await?)?;
    tokio::fs::write(target.join("user.txt"), "Unowned content").await?;
    assert!(publisher.render(&plan, temp.path()).await.is_err());
    assert_eq!(
        tokio::fs::read_to_string(target.join("user.txt")).await?,
        "Unowned content"
    );
    Ok(())
}

#[tokio::test]
async fn missing_local_heading_links_fail_before_publication()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let (dataset, plan, _) = plans(
        "[Missing heading](#does-not-exist)",
        &temp.path().join("project.ttl"),
    )
    .await?;
    assert!(
        Publisher::new(dataset)
            .render(&plan, temp.path())
            .await
            .is_err()
    );
    assert!(!tokio::fs::try_exists(temp.path().join("build/spec")).await?);
    Ok(())
}

#[tokio::test]
async fn publication_jobs_are_recorded_in_the_shared_graph_without_changing_source_revision()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let project = "urn:example:ontology-project";
    let (dataset, plan, _) = plans(
        "Recorded publication.",
        &temp.path().join("project.ttl"),
    )
    .await?;
    let before = Assembler::new(dataset.clone()).assemble(project).await?;
    let outcome = Publisher::new(dataset.clone())
        .render(&plan, temp.path())
        .await?;
    let PublishOutcome::Published(publication) = outcome else {
        anyhow::bail!("unexpected superseded output")
    };
    let mut rows = dataset.select(format!("SELECT ?status WHERE {{ GRAPH <urn:grapharch:publications> {{ <urn:grapharch:publication:{}> <https://ekgf.org/ontology/grapharch#status> ?status }} }}", publication.manifest.job)).await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("publication job missing from graph"))?;
    assert_eq!(
        row["status"],
        Literal::new_simple_literal("published").into()
    );
    assert_eq!(
        before,
        Assembler::new(dataset).assemble(project).await?
    );
    Ok(())
}

#[tokio::test]
async fn markdown_heading_depth_is_relative_to_its_component_without_numbering_gaps()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let (dataset, _, plan) = plans(
        "## First heading\n\nA paragraph.\n\n### Nested heading\n\nMore prose.",
        &temp.path().join("project.ttl"),
    )
    .await?;
    let target =
        directory(Publisher::new(dataset).render(&plan, temp.path()).await?)?;
    let source = tokio::fs::read_to_string(target.join("main.typ")).await?;
    assert!(source.contains("#heading(level: 2)[#text(\"First heading\")"));
    assert!(source.contains("#heading(level: 3)[#text(\"Nested heading\")"));
    Ok(())
}

#[tokio::test]
async fn skipped_heading_levels_are_rejected_instead_of_publishing_zero_numbered_sections()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let (dataset, plan, _) = plans(
        "# Parent\n\n### Skipped level\n\nProse.",
        &temp.path().join("project.ttl"),
    )
    .await?;
    assert!(
        Publisher::new(dataset)
            .render(&plan, temp.path())
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn shared_html_and_pdf_render_gfm_callouts_without_literal_markers()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let (dataset, html, pdf) = plans(
        "> [!NOTE]\n> Keep **this guidance**.\n\n- [x] Completed task\n- [ ] \
         Pending task",
        &temp.path().join("source.ttl"),
    )
    .await?;
    let publisher = Publisher::new(dataset);
    let PublishOutcome::Published(html) =
        publisher.render(&html, temp.path()).await?
    else {
        panic!("expected HTML")
    };
    let output =
        tokio::fs::read_to_string(html.directory.join("index.html")).await?;
    assert!(output.contains("markdown-alert-note"));
    assert!(output.contains("callout-title"));
    assert!(output.contains("task-list-item"));
    assert!(!output.contains("[!NOTE]"));
    let PublishOutcome::Published(pdf) =
        publisher.render(&pdf, temp.path()).await?
    else {
        panic!("expected PDF")
    };
    let typst =
        tokio::fs::read_to_string(pdf.directory.join("main.typ")).await?;
    assert!(typst.contains("Note"));
    assert!(!typst.contains("[!NOTE]"));
    assert!(
        tokio::fs::read(pdf.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}
