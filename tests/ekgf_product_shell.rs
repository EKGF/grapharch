use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn ekgf_products_share_the_same_community_shell() -> Result<()> {
    let mut expected = None;
    for (name, marketing) in [
        ("DPROD", true),
        ("GraphArch", true),
        ("EKG Method", false),
        ("EKG Catalog", false),
        ("EKG Maturity Model", false),
        ("EKG Principles", false),
        ("EKG Ontologies", false),
    ] {
        let source = tempfile::tempdir()?;
        let output = tempfile::tempdir()?;
        let definitions = source.path().join("project.ttl");
        let style = if marketing { "ekgf-project" } else { "ekgf" };
        let layout = if marketing {
            ""
        } else {
            "; ga:layout \"documentation\""
        };
        tokio::fs::write(&definitions, format!(r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "{name}"; ga:target <urn:target> .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
<urn:scope> a ga:Scope . <urn:profile> a ga:Profile; ga:style "{style}" {layout} .
"#)).await?;
        tokio::fs::write(
            source.path().join("index.md"),
            format!("# {name}\n\nProduct-owned content.\n"),
        )
        .await?;
        let built = website::collect(
            source.path(),
            &definitions,
            "urn:project",
            "urn:target",
            2,
        )
        .await?;
        let PublishOutcome::Published(publication) =
            Publisher::new(built.dataset)
                .render(&built.plan, output.path())
                .await?
        else {
            panic!("published")
        };
        let html =
            tokio::fs::read_to_string(publication.directory.join("index.html"))
                .await?;
        let doc = scraper::Html::parse_document(&html);
        let community = doc
            .select(&scraper::Selector::parse(".community-header-row").unwrap())
            .next()
            .expect("every EKGF product uses the shared community header")
            .inner_html();
        let footer = doc
            .select(&scraper::Selector::parse(".ekgf-footer").unwrap())
            .next()
            .expect("shared footer")
            .inner_html();
        assert!(community.contains("About"));
        assert!(community.contains("color-scheme-toggle"));
        assert!(html.contains(name));
        if let Some((header, previous_footer)) = &expected {
            assert_eq!(&community, header, "{name} header");
            assert_eq!(&footer, previous_footer, "{name} footer");
        } else {
            expected = Some((community, footer));
        }
    }
    Ok(())
}
