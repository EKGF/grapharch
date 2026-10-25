#[path = "support/mod.rs"]
mod specification_assertions;

#[path = "../examples/support/mod.rs"]
mod support;

use grapharch::publication::{
    dataset::Dataset,
    render::{PublishOutcome, Publisher},
};

#[tokio::test]
async fn authored_showcase_publishes_three_targets_from_an_exported_graph()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("project.ttl");
    tokio::fs::copy("examples/showcase/project.ttl", &source).await?;
    let (dataset, plans) = support::authored(&source).await?;
    tokio::fs::remove_file(source).await?;
    let archive = temp.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications =
        support::publish(&Publisher::new(restored), &plans, temp.path())
            .await?;
    assert_eq!(publications.len(), 3);
    let manifests: Vec<_> = publications.iter().map(|p| &p.manifest).collect();
    assert!(
        manifests
            .iter()
            .all(|m| m.input_revision == manifests[0].input_revision)
    );
    let specification =
        tokio::fs::read_to_string(temp.path().join("specification/index.html"))
            .await?;
    let reader =
        tokio::fs::read_to_string(temp.path().join("reader/index.html"))
            .await?;
    let typst =
        tokio::fs::read_to_string(temp.path().join("review/main.typ")).await?;
    for content in [&specification, &reader, &typst] {
        assert!(content.contains("One graph, several publications"));
        assert!(content.contains("Publication contract"));
        assert!(content.contains("Documentable subject"));
    }
    assert!(
        specification
            .rfind("Appendix A. Publication contract")
            .unwrap() <
            specification.rfind("Glossary").unwrap()
    );
    assert!(
        reader.rfind("Glossary").unwrap() <
            reader.rfind("Appendix A. Publication contract").unwrap()
    );
    assert!(
        tokio::fs::read(temp.path().join("review/document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn mixed_showcase_collects_native_sources_and_documents_owl_before_rendering()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let sources = temp.path().join("sources");
    tokio::fs::create_dir(&sources).await?;
    for file in ["project.ttl", "introduction.md", "ontology.ttl", "query.rq"] {
        tokio::fs::copy(
            format!("examples/mixed-sources/{file}"),
            sources.join(file),
        )
        .await?;
    }
    let (dataset, plans) = support::mixed(&sources).await?;
    tokio::fs::remove_dir_all(sources).await?;
    let publications =
        support::publish(&Publisher::new(dataset), &plans, temp.path()).await?;
    assert_eq!(publications.len(), 2);
    let html =
        tokio::fs::read_to_string(temp.path().join("website/index.html"))
            .await?;
    let typst =
        tokio::fs::read_to_string(temp.path().join("review/main.typ")).await?;
    for content in [&html, &typst] {
        assert!(content.contains("Authored guidance from a Markdown file"));
        assert!(content.contains("A governed collection of data"));
        assert!(content.contains("SELECT ?product"));
    }
    assert!(
        tokio::fs::read(temp.path().join("review/document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn specification_renders_publication_metadata_and_numbered_navigation_from_kg()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("project.ttl");
    tokio::fs::copy("examples/specification/project.ttl", &source).await?;
    let (dataset, plans) = support::specification(&source).await?;
    tokio::fs::remove_file(source).await?;
    let publications =
        support::publish(&Publisher::new(dataset), &plans, temp.path()).await?;
    assert_eq!(publications[0].manifest.renderer, "specification");
    let html =
        tokio::fs::read_to_string(temp.path().join("specification/index.html"))
            .await?;
    for expected in [
        "Editor&#39;s Draft",
        "24 October 2026",
        "This version:",
        "Latest published version:",
        "Editor:",
        "Enterprise Knowledge Graph Forum",
        "Alex Example",
        "Abstract",
        "Status of This Document",
        "Table of Contents",
        "1. Introduction",
        "1.1 Data model",
        "1.1.1 Identity",
        "A. Change history",
        "role=\"doc-toc\"",
        "class=\"self-link\"",
    ] {
        assert!(html.contains(expected), "missing {expected}");
    }
    assert!(
        html.find("</header>").unwrap() < html.find("<nav id=\"toc\"").unwrap(),
        "publication header must precede inline navigation on narrow screens"
    );
    let toc_start = html.find("<nav id=\"toc\"").unwrap();
    let toc_end = html[toc_start..].find("</nav>").unwrap() + toc_start;
    assert!(!html[toc_start..toc_end].contains("Abstract"));
    assert!(!html[toc_start..toc_end].contains("Status of This Document"));
    specification_assertions::assert_specification_scripts(&html);
    assert!(!html.contains("W3C Recommendation"));
    assert!(!html.contains("https://www.w3.org/StyleSheets"));
    assert!(html.contains("&lt;example&gt;"));
    assert!(html.contains("rel=\"license\""));
    assert!(html.contains("href=\"document.pdf\""));
    assert!(html.contains("aria-label=\"Download PDF\""));
    assert!(
        publications[0]
            .manifest
            .files
            .iter()
            .any(|file| file == "document.pdf")
    );
    let pdf =
        tokio::fs::read(temp.path().join("specification/document.pdf")).await?;
    assert!(pdf.starts_with(b"%PDF-"));
    let typst =
        tokio::fs::read_to_string(temp.path().join("specification/main.typ"))
            .await?;
    assert!(typst.contains("Documentation Publication Model 1.0"));
    assert!(typst.contains("Publication requirements"));
    Ok(())
}

#[tokio::test]
async fn incomplete_or_unsafe_specification_metadata_preserves_previous_output()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("project.ttl");
    let fixture =
        tokio::fs::read_to_string("examples/specification/project.ttl").await?;
    tokio::fs::write(&source, &fixture).await?;
    let (dataset, plans) = support::specification(&source).await?;
    support::publish(&Publisher::new(dataset), &plans, temp.path()).await?;
    let output = temp.path().join("specification/index.html");
    let original = tokio::fs::read(&output).await?;
    for (before, after, message) in [
        (
            "ga:publicationStatus \"Editor's Draft\" ;",
            "",
            "publicationStatus",
        ),
        ("2026-10-24", "2026-02-30", "publicationDate"),
        ("#2b5f9f", "red", "accentColor"),
        (
            "data:image/png;base64,",
            "data:image/svg+xml;base64,",
            "PNG",
        ),
        (
            "https://example.org/spec/2026-10-24/",
            "javascript:alert(1)",
            "HTTP",
        ),
    ] {
        tokio::fs::write(&source, fixture.replace(before, after)).await?;
        let (dataset, plans) = support::specification(&source).await?;
        let outcome =
            Publisher::new(dataset).render(&plans[0], temp.path()).await;
        match outcome {
            Err(error) => {
                assert!(
                    format!("{error:#}").contains(message),
                    "{error:#}"
                )
            },
            Ok(PublishOutcome::Published(_)) => {
                panic!("invalid metadata was published")
            },
            Ok(PublishOutcome::Superseded) => panic!("unexpected supersession"),
        }
        assert_eq!(tokio::fs::read(&output).await?, original);
    }
    Ok(())
}

#[tokio::test]
async fn specification_authority_controls_branding_without_changing_renderer()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("project.ttl");
    let fixture =
        tokio::fs::read_to_string("examples/specification/project.ttl").await?;
    for (name, short, color) in [
        (
            "Enterprise Knowledge Graph Forum",
            "EKGF",
            "#2b5f9f",
        ),
        ("Example Standards Authority", "ESA", "#664499"),
    ] {
        let text = fixture
            .replace("Enterprise Knowledge Graph Forum", name)
            .replace("\"EKGF\"", &format!("\"{short}\""))
            .replace("#2b5f9f", color);
        tokio::fs::write(&source, text).await?;
        let (dataset, plans) = support::specification(&source).await?;
        support::publish(&Publisher::new(dataset), &plans, temp.path()).await?;
        let html = tokio::fs::read_to_string(
            temp.path().join("specification/index.html"),
        )
        .await?;
        assert!(html.contains(&format!("{short} Editor&#39;s Draft")));
        assert!(html.contains(&format!("--authority-color:{color}")));
        assert!(html.contains("class=\"authority-logo\""));
        assert!(html.contains("src=\"data:image/png;base64,"));
        assert!(html.contains(&format!("alt=\"{name}\"")));
        assert!(html.contains("class=\"toc-number\""));
        assert!(html.contains("class=\"toc-label\""));
        assert!(!html.contains("W3C Recommendation"));
    }
    Ok(())
}

#[tokio::test]
async fn w3c_showcase_includes_its_collected_authority_assets()
-> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let (dataset, plans) = support::w3c(std::path::Path::new(
        "examples/w3c-rdf12-n-triples",
    ))
    .await?;
    support::publish(
        &Publisher::new(dataset).with_failure_mode(
            grapharch::publication::diagnostics::FailureMode::KeepGoing,
        ),
        &plans,
        temp.path(),
    )
    .await?;
    let html =
        tokio::fs::read_to_string(temp.path().join("specification/index.html"))
            .await?;
    assert!(html.contains("W3C Working Draft"));
    assert!(html.contains("alt=\"World Wide Web Consortium\""));
    assert!(html.contains("data:image/png;base64,"));
    Ok(())
}
