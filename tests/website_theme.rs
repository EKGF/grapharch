use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn ekgf_website_theme_is_opt_in_and_survives_source_removal() -> Result<()>
{
    for theme in [None, Some("ekgf")] {
        let input = tempfile::tempdir()?;
        let settings = input.path().join("project.ttl");
        let style = theme
            .map(|value| format!("; ga:style \"{value}\""))
            .unwrap_or_default();
        tokio::fs::write(&settings, format!(r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> . @prefix dct: <http://purl.org/dc/terms/> .
<urn:brand:project> a ga:Project; dct:title "Grapharch"; ga:scope <urn:brand:scope>; ga:target <urn:brand:target> .
<urn:brand:scope> a ga:Scope . <urn:brand:profile> a ga:Profile {style} .
<urn:brand:target> a ga:Target; dct:title "Docs"; ga:scope <urn:brand:scope>; ga:profile <urn:brand:profile>; ga:renderer "website"; ga:destination "site" .
"#)).await?;
        tokio::fs::write(
            input.path().join("index.md"),
            "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
             urn:brand:home\n---\n# Grapharch\n\nDocumentation in the EKGF \
             community.\n",
        )
        .await?;
        let collected = website::collect(
            input.path(),
            &settings,
            "urn:brand:project",
            "urn:brand:target",
            2,
        )
        .await?;
        let temporary = tempfile::tempdir()?;
        let archive = temporary.path().join("kg.nq");
        collected.dataset.export(archive.clone()).await?;
        input.close()?;
        let restored = Dataset::memory()?;
        restored.import(archive).await?;
        let output = std::env::var_os("GRAPHARCH_EKGF_REVIEW")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| temporary.path().join("output"));
        let output = output.join(theme.unwrap_or("default"));
        let PublishOutcome::Published(publication) = Publisher::new(restored)
            .render(&collected.plan, &output)
            .await?
        else {
            panic!("publication required")
        };
        let home =
            tokio::fs::read_to_string(publication.directory.join("index.html"))
                .await?;
        let document = scraper::Html::parse_document(&home);
        let logo = scraper::Selector::parse(
            ".site-header a.ekgf-identity[href='https://ekgf.org/'] svg",
        )
        .unwrap();
        let version =
            scraper::Selector::parse(".site-header .brand-version").unwrap();
        assert!(document.select(&version).next().is_some());
        if theme.is_some() {
            assert!(
                document.select(&logo).next().is_some(),
                "EKGF identity must use the actual graph mark"
            );
            assert!(home.contains("class=\"theme-ekgf\""));
            let css = tokio::fs::read_to_string(
                publication.directory.join("style.css"),
            )
            .await?;
            assert!(css.contains("#4051b5") && css.contains("#ff6f00"));
            assert!(publication.directory.join("brand/roboto.ttf").is_file());
            assert!(
                tokio::fs::read_to_string(
                    publication.directory.join("brand/NOTICE.txt")
                )
                .await?
                .contains("CC BY-SA 4.0")
            );
        } else {
            assert!(
                document.select(&logo).next().is_none(),
                "Other publishers must not inherit EKGF branding"
            );
            assert!(!publication.directory.join("brand/roboto.ttf").exists());
        }
    }
    Ok(())
}
