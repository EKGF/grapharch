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
    assert!(doc.select(&select(".json-key")).next().is_some());
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(typst.contains("#ga-example[#raw(block: true, lang: \"json\""));
    assert_eq!(typst.matches("#ga-callout(\"Note\"").count(), 2);
    assert!(
        tokio::fs::metadata(publication.directory.join("document.pdf"))
            .await?
            .len() >
            1000
    );
    Ok(())
}
