#[path = "support/frontmatter.rs"]
mod frontmatter;

use {
    anyhow::Result,
    oxigraph::{io::RdfFormat, store::Store},
    pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd},
};

#[tokio::test]
async fn sparql_guides_use_endpoint_terminology() -> Result<()> {
    for path in [
        "features/README.md",
        "features/knowledge-graph.md",
        "features/sources/endpoints/sparql.md",
        "usage/sparql-endpoints.md",
        "other/architecture/README.md",
        "other/architecture/deliverables.md",
    ] {
        let body = tokio::fs::read_to_string(format!("docs/{path}")).await?;
        assert!(
            !body.to_lowercase().contains("sparql service"),
            "{path}: call it a SPARQL endpoint"
        );
    }
    Ok(())
}

#[tokio::test]
async fn documentable_guides_have_examples_with_source_and_generated_output()
-> Result<()> {
    for path in [
        "documentable-things/authored-documentation.md",
        "documentable-things/ontologies.md",
        "documentable-things/ontology-families.md",
        "documentable-things/shacl-shapes.md",
        "documentable-things/published-specifications.md",
        "usage/skos-taxonomies.md",
        "usage/use-case-trees.md",
        "documentable-things/sparql-queries.md",
        "documentable-things/legal-preambles.md",
    ] {
        let body = tokio::fs::read_to_string(format!("docs/{path}")).await?;
        let mut snippet = String::new();
        let mut language = String::new();
        let mut examples = 0;
        let mut source = false;
        let mut output = false;
        let mut collector = false;
        for event in Parser::new(&body) {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(kind))) => {
                    language = kind.into_string();
                    snippet.clear();
                    examples += 1;
                },
                Event::Text(text) if !language.is_empty() => {
                    snippet.push_str(&text);
                },
                Event::End(TagEnd::CodeBlock) => {
                    if language == "turtle" {
                        Store::new()?.load_from_reader(
                            RdfFormat::Turtle,
                            snippet.as_bytes(),
                        )?;
                    }
                    language.clear();
                },
                Event::Start(Tag::Link { dest_url, .. }) => {
                    source |= dest_url.starts_with("https://github.com/");
                    output |= dest_url.contains("/showcases/artifacts/");
                    collector |= dest_url.starts_with("../collectors/");
                },
                _ => {},
            }
        }
        assert!(
            examples > 0,
            "{path} needs a concrete input example"
        );
        assert!(
            source,
            "{path} needs an inspectable example source"
        );
        assert!(
            output,
            "{path} needs a direct generated-output link"
        );
        assert!(
            collector,
            "{path} must identify its supporting collectors"
        );
    }
    let markdown = tokio::fs::read_to_string(
        "docs/documentable-things/authored-documentation.md",
    )
    .await?;
    #[derive(serde::Deserialize)]
    struct Guide {
        title: String,
    }
    let (metadata, _) = frontmatter::parse(&markdown)?;
    assert_eq!(
        serde_yaml::from_str::<Guide>(metadata)?.title,
        "Markdown pages and guides"
    );
    Ok(())
}
