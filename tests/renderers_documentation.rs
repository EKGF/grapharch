#[path = "support/frontmatter.rs"]
mod frontmatter;

use {
    anyhow::Result,
    pulldown_cmark::{Event, Parser, Tag},
    std::path::Path,
};

#[tokio::test]
async fn renderer_catalogue_matches_implemented_modes_and_links() -> Result<()>
{
    let root = Path::new("docs/renderers");
    let index = tokio::fs::read_to_string(root.join("README.md")).await?;
    for (file, mode) in [
        ("website", "website"),
        ("specification", "specification"),
        ("specification-pages", "specification-pages"),
        ("bookshelf", "bookshelf"),
        ("pdf", "typst"),
        ("html", "html"),
    ] {
        assert!(index.contains(&format!("({file}.md)")));
        let source =
            tokio::fs::read_to_string(root.join(format!("{file}.md"))).await?;
        let (metadata, body) = frontmatter::parse(&source)?;
        assert!(metadata.contains("parent: Renderers"));
        assert!(metadata.contains(&format!("permalink: /renderers/{file}/")));
        assert!(body.contains(&format!("ga:renderer \"{mode}\"")));
        for event in Parser::new(body) {
            if let Event::Start(Tag::Link { dest_url, .. }) = event {
                let path = dest_url.split('#').next().unwrap();
                if !path.is_empty() && url::Url::parse(path).is_err() {
                    assert!(
                        tokio::fs::try_exists(root.join(path)).await?,
                        "missing {path}"
                    );
                }
            }
        }
    }
    Ok(())
}
