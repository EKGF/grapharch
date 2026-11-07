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

#[tokio::test]
async fn authored_landing_and_documentation_share_one_theme() -> Result<()> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
<urn:p> a ga:Project; ga:target <urn:t> .
<urn:t> a ga:Target; ga:scope <urn:s>; ga:profile <urn:f>; ga:renderer "website"; ga:destination "site" .
<urn:s> a ga:Scope . <urn:f> a ga:Profile; ga:style "ekgf"; ga:layout "documentation" .
"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ngrapharch:\n  hero: landing\n  feature_sections: \
         [benefits]\n---\n# Product\n\nShared introduction.\n\n## \
         Benefits\n\n### One source\n\nMany outputs.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("guide.md"),
        "# Guide\n\nDetailed instructions.\n",
    )
    .await?;
    let built =
        website::collect(source.path(), &definitions, "urn:p", "urn:t", 2)
            .await?;
    let archive = output.path().join("graph.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let dataset = grapharch::publication::dataset::Dataset::memory()?;
    dataset.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("published")
    };
    let home =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let guide = tokio::fs::read_to_string(
        publication.directory.join("guide/index.html"),
    )
    .await?;
    assert!(home.contains("project-hero-landing"));
    assert!(home.contains("project-section-features"));
    assert!(!guide.contains("project-hero-landing"));
    assert!(guide.contains("class=\"site-navigation\""));
    for html in [&home, &guide] {
        assert!(html.contains("community-header-row"));
        assert!(html.contains("ekgf-footer"));
    }
    Ok(())
}
