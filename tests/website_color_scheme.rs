use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn community_color_scheme_is_shared_by_pages_and_reports_after_restore()
-> Result<()> {
    for style in ["reader", "ekgf", "ekgf-project"] {
        let source = tempfile::tempdir()?;
        let settings = source.path().join("project.ttl");
        tokio::fs::write(&settings, format!(r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> . @prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "GraphArch"; ga:scope <urn:scope>; ga:target <urn:target>; ga:repository <https://github.com/EKGF/grapharch> .
<urn:scope> a ga:Scope . <urn:profile> a ga:Profile; ga:style "{style}" .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
<urn:quality> dct:title "First title"@en, "Second title"@en .
"#)).await?;
        tokio::fs::write(
            source.path().join("index.md"),
            r#"---
title: Home
permalink: /
---
# GraphArch

Readable documentation with [a link](nested.md) and `inline code`.

[Read the guide](nested.md)

## Examples

> [!NOTE]
> A note with a [link](nested.md).

> [!WARNING]
> A warning that remains readable.

```json example
{"name": "Bønds", "active": true, "count": 3}
```

| Property | Value |
| --- | --- |
| Example | A table entry |

### A card

Project cards share the selected color scheme.
"#,
        )
        .await?;
        tokio::fs::write(
            source.path().join("nested.md"),
            "---\ntitle: Nested\npermalink: /guide/nested/\n---\n# \
             Nested\n\n[Home](index.md)\n",
        )
        .await?;
        let collected = website::collect(
            source.path(),
            &settings,
            "urn:project",
            "urn:target",
            2,
        )
        .await?;
        let temporary = tempfile::tempdir()?;
        let archive = temporary.path().join("kg.nq");
        collected.dataset.export(archive.clone()).await?;
        source.close()?;
        let restored = Dataset::memory()?;
        restored.import(archive).await?;
        let output = std::env::var_os("GRAPHARCH_COLOR_REVIEW")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| temporary.path().join("output"))
            .join(style);
        let PublishOutcome::Published(publication) = Publisher::new(restored)
            .render(&collected.plan, &output)
            .await?
        else {
            panic!("publication required")
        };
        for route in [
            "index.html",
            "guide/nested/index.html",
            "provenance.html",
            "quality-issues.html",
        ] {
            let html =
                tokio::fs::read_to_string(publication.directory.join(route))
                    .await?;
            let document = scraper::Html::parse_document(&html);
            let controls = document
                .select(
                    &scraper::Selector::parse(
                        "button.color-scheme-toggle[type='button']",
                    )
                    .unwrap(),
                )
                .count();
            if style == "reader" {
                assert_eq!(controls, 0);
                assert!(!html.contains("data-color-scheme-controller"));
            } else {
                assert_eq!(controls, 1, "{style}/{route} theme toggle");
                let script = html
                    .find("<script data-color-scheme-controller>")
                    .expect("pre-paint controller");
                assert!(
                    script < html.find("<link rel=\"stylesheet\"").unwrap()
                );
                assert!(html.contains("grapharch:color-scheme"));
            }
        }
    }
    Ok(())
}
