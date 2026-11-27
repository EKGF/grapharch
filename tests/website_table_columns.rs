//! A narrow first table column keeps a short identifier, such as the link
//! `0002`, on one line in every website theme.
use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

async fn stylesheet(theme: &str) -> Result<String> {
    let source = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(
        &definitions,
        format!(
            r#"@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Records"; ga:target <urn:target> .
<urn:scope> a ga:Scope .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer ga:WebsiteRenderer; ga:destination "site" .
<urn:profile> a ga:Profile; ga:theme ga:{theme} .
"#
        ),
    )
    .await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Records\npermalink: /\n---\n# Records\n\n| Record | \
         Decision |\n| --- | --- |\n| [0002](https://example.org/0002) | A \
         long explanation of the second decision that fills the row. |\n",
    )
    .await?;
    let collected = website::collect(
        source.path(),
        &definitions,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    collected.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&collected.plan, &output.path().join("output"))
        .await?
    else {
        panic!("publication expected")
    };
    Ok(
        tokio::fs::read_to_string(publication.directory.join("style.css"))
            .await?,
    )
}

#[tokio::test]
async fn table_cells_never_shrink_below_their_longest_word() -> Result<()> {
    for theme in ["ReaderTheme", "EkgfProjectTheme"] {
        let css = stylesheet(theme).await?;
        // Article text may break anywhere, which lets an automatic table
        // column shrink to one character; cells opt back into whole words.
        assert!(
            css.contains("article{overflow-wrap:anywhere}"),
            "fixture premise: article text breaks anywhere"
        );
        assert!(
            css.contains("td{overflow-wrap:break-word;min-width:2.5em}"),
            "{theme}: table cells must keep short tokens whole"
        );
    }
    Ok(())
}
