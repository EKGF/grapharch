use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn ekgf_header_uses_community_row_and_source_owned_project_links()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "GraphArch"; ga:scope <urn:scope>; ga:target <urn:target>; ga:repository <https://github.com/EKGF/grapharch> .
<urn:scope> a ga:Scope .
<urn:target> a ga:Target; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site" .
<urn:profile> a ga:Profile; ga:style "ekgf"; ga:headerLink <urn:spec>, <urn:guide> .
<urn:spec> dct:title "Specification"; ga:url "spec/develop/"; ga:order 2 .
<urn:guide> dct:title "Get started"; ga:url "https://example.org/guide/"; ga:order 1 .
"#).await?;
    tokio::fs::write(
        source.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\n---\n# GraphArch\n\nHome.\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("nested.md"),
        "---\ntitle: Nested\npermalink: /guide/nested/\n---\n# \
         Nested\n\nGuide.\n",
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
    let directory = std::env::var_os("GRAPHARCH_HEADER_REVIEW")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| output.path().join("output"));
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&collected.plan, &directory)
        .await?
    else {
        panic!("publication expected")
    };
    for route in ["index.html", "guide/nested/index.html", "provenance.html"] {
        let html = tokio::fs::read_to_string(publication.directory.join(route))
            .await?;
        let document = scraper::Html::parse_document(&html);
        let links = document
            .select(
                &scraper::Selector::parse("header .ekgf-header-navigation a")
                    .unwrap(),
            )
            .collect::<Vec<_>>();
        assert_eq!(
            links
                .iter()
                .map(|link| link.text().collect::<String>())
                .collect::<Vec<_>>(),
            [
                "About",
                "Quadrants",
                "Resources",
                "Membership",
                "Contact",
                "Get started",
                "Specification"
            ]
        );
        let base =
            url::Url::parse(&format!("https://example.org/docs/{route}"))?;
        assert_eq!(
            base.join(links.last().unwrap().value().attr("href").unwrap())?
                .as_str(),
            "https://example.org/docs/spec/develop/"
        );
        for selector in [
            "header .community-header-row .ekgf-identity",
            "header .community-header-row .source-link",
            "header .community-header-row .omg-identity",
            "header .project-identity-row .brand-version",
            "header .community-menu summary",
        ] {
            assert!(
                document
                    .select(&scraper::Selector::parse(selector).unwrap())
                    .next()
                    .is_some(),
                "missing {selector} in {route}"
            );
        }
    }
    Ok(())
}
