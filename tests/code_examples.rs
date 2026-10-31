#[path = "support/definitions.rs"]
mod definitions;

use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        showcases,
    },
};

const EXAMPLE: &str = "```json example\n{\"title\": \"UK Bonds\", \"active\": \
                       true}\n```\n\n> [!NOTE]\n> Two contexts describe the \
                       same graph.\n\n> [!NOTE]\n> Shapes describe supported \
                       properties.\n";

#[tokio::test]
async fn authored_example_and_notes_survive_kg_restore_in_html_and_pdf()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = definitions::set_property(
        include_str!("../examples/specification/project.ttl"),
        "urn:specification:model-body",
        "https://ekgf.org/ontology/grapharch#body",
        definitions::literal(EXAMPLE),
    )?;
    tokio::fs::write(&path, fixture).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let saved = tempfile::tempdir()?;
    dataset.export(saved.path().join("source.nq")).await?;
    source.close()?;
    let restored = grapharch::publication::dataset::Dataset::memory()?;
    restored.import(saved.path().join("source.nq")).await?;
    let output = std::env::var_os("GRAPHARCH_EXAMPLE_PREVIEW")
        .map(std::path::PathBuf::from);
    let temporary = tempfile::tempdir()?;
    let root = output.as_deref().unwrap_or(temporary.path());
    let PublishOutcome::Published(publication) =
        Publisher::new(restored).render(&plans[0], root).await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let doc = scraper::Html::parse_document(&html);
    let select = |query| scraper::Selector::parse(query).unwrap();
    assert_eq!(
        doc.select(&select("figure.code-example")).count(),
        1
    );
    assert_eq!(
        doc.select(&select("blockquote.markdown-alert-note"))
            .count(),
        2
    );
    assert!(doc.select(&select("code span[style]")).next().is_some());
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(typst.contains("#ga-example[#ga-code(("));
    assert_eq!(typst.matches("#ga-callout(\"Note\"").count(), 2);
    assert!(
        tokio::fs::metadata(publication.directory.join("document.pdf"))
            .await?
            .len() >
            1000
    );
    Ok(())
}

#[tokio::test]
async fn multilingual_showcase_renders_from_restored_graph_without_sources()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    tokio::fs::write(
        &path,
        include_str!("../examples/code-highlighting/project.ttl"),
    )
    .await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let saved = tempfile::tempdir()?;
    dataset.export(saved.path().join("source.nq")).await?;
    source.close()?;
    let restored = grapharch::publication::dataset::Dataset::memory()?;
    restored.import(saved.path().join("source.nq")).await?;
    let preview = std::env::var_os("GRAPHARCH_CODE_PREVIEW")
        .map(std::path::PathBuf::from);
    let temporary = tempfile::tempdir()?;
    let root = preview.as_deref().unwrap_or(temporary.path());
    let PublishOutcome::Published(publication) =
        Publisher::new(restored).render(&plans[0], root).await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let doc = scraper::Html::parse_document(&html);
    for language in [
        "python",
        "bash",
        "javascript",
        "typescript",
        "json",
        "json-ld",
        "sparql",
    ] {
        let selector =
            scraper::Selector::parse(&format!("code.language-{language}"))
                .unwrap();
        let code = doc
            .select(&selector)
            .next()
            .unwrap_or_else(|| panic!("missing {language}"));
        let colors: std::collections::HashSet<_> = code
            .select(&scraper::Selector::parse("span[style]").unwrap())
            .map(|span| span.attr("style").unwrap())
            .collect();
        assert!(
            colors.len() >= 3,
            "{language} needs meaningful syntax colours"
        );
    }
    let select = |s| scraper::Selector::parse(s).unwrap();
    let python = doc
        .select(&select("code.language-python"))
        .next()
        .unwrap()
        .text()
        .collect::<String>();
    assert!(python.contains("A second line stays inside this string."));
    let json = doc
        .select(&select("code.language-json"))
        .next()
        .unwrap()
        .text()
        .collect::<String>();
    assert!(json.contains("<script>& </code>"));
    assert!(doc.select(&select("code script")).next().is_none());
    let sparql = doc
        .select(&select("code.language-sparql"))
        .next()
        .unwrap()
        .text()
        .collect::<String>();
    assert!(sparql.contains("SELECT ?dataset $title WHERE {"));
    assert!(sparql.contains("<http://purl.org/dc/terms/>"));
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert_eq!(typst.matches("#ga-example[#ga-code((").count(), 7);
    assert!(typst.contains("$title"));
    assert!(
        tokio::fs::metadata(publication.directory.join("document.pdf"))
            .await?
            .len() >
            1000
    );
    Ok(())
}
