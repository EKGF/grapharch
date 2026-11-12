use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn project_pages_render_source_owned_sections_without_documentation_chrome()
-> Result<()> {
    let input = tempfile::tempdir()?;
    let settings = input.path().join("project.ttl");
    tokio::fs::write(&settings, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Example project"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope .
<urn:profile> a ga:Profile; ga:theme ga:EkgfProjectTheme .
<urn:target> a ga:Target; dct:title "Project website"; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
"#).await?;
    tokio::fs::write(
        input.path().join("index.md"),
        "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: urn:home\n  hero: landing\n  \
         panel_sections: [benefits]\n  feature_sections: [benefits]\n  resource_sections: [next-steps]\n  \
         card_icons:\n    first-benefit: {name: network, tone: primary}\n    \
         second-benefit: {name: shield, tone: accent}\n    article: {name: book-open, tone: primary}\n---\n# Example \
         project\n\nAn introduction.\n\n[Get started](guide.md)\n\n## \
         Benefits\n\n### First benefit\n\nA source-owned explanation.\n\n### \
         Second benefit\n\nAnother explanation.\n\n## Next steps\n\n### Article\n\nArticle summary.\n\n[Read the \
         guide](https://example.org/article)\n",
    )
    .await?;
    tokio::fs::write(
        input.path().join("guide.md"),
        "---\ntitle: Guide\npermalink: /guide/\ngrapharch:\n  subject: \
         urn:guide\n  card_icons:\n    step: {name: rocket, tone: primary}\n---\n[Home](index.md)\n\n## Step\n\nGuide text.\n\n[Read more](https://example.org/more)\n",
    )
    .await?;
    let collected = website::collect(
        input.path(),
        &settings,
        "urn:project",
        "urn:target",
        2,
    )
    .await?;
    let export = tempfile::tempdir()?;
    collected
        .dataset
        .export(export.path().join("kg.nq"))
        .await?;
    input.close()?;
    let restored = Dataset::memory()?;
    restored.import(export.path().join("kg.nq")).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&collected.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    let html =
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?;
    let doc = scraper::Html::parse_document(&html);
    let count = |selector| {
        doc.select(&scraper::Selector::parse(selector).unwrap())
            .count()
    };
    assert_eq!(
        count(".project-section-resources h2#next-steps"),
        1
    );
    assert_eq!(
        count(".project-section-resources .project-card-content h3#article"),
        1
    );
    assert_eq!(
        count(".project-card-action a svg[aria-hidden=\"true\"]"),
        1
    );
    assert_eq!(count(".project-hero.project-hero-landing"), 1);
    assert_eq!(count(".project-identity-row"), 0);
    assert_eq!(count(".project-section"), 2);
    assert_eq!(count(".project-section-panel"), 1);
    assert_eq!(count(".project-section-panel h2#benefits"), 1);
    assert_eq!(count(".project-section-panel h2#next-steps"), 0);
    assert_eq!(count(".project-card"), 3);
    assert_eq!(count(".project-section-features h2#benefits"), 1);
    assert_eq!(
        count(".project-card-icon[aria-hidden=\"true\"]"),
        3
    );
    assert_eq!(count(".project-card-icon-primary svg"), 2);
    assert_eq!(count(".project-card-icon-accent svg"), 1);
    assert_eq!(count(".project-hero .project-actions"), 1);
    assert_eq!(
        count(".project-actions a[href=\"https://example.org/intro\"]"),
        0
    );
    assert_eq!(
        count(".site-navigation, .page-outline, .breadcrumbs, .page-neighbors"),
        0
    );
    assert!(html.contains("href=\"guide/\""));
    let nested = tokio::fs::read_to_string(
        publication.directory.join("guide/index.html"),
    )
    .await?;
    assert!(nested.contains("href=\"../\""));
    assert!(nested.contains("project-hero-introduction"));
    assert!(nested.contains("project-section-heading"));
    assert!(nested.contains("project-section-actions"));
    let guide = scraper::Html::parse_document(&nested);
    assert_eq!(
        guide
            .select(&scraper::Selector::parse(".project-hero h1").unwrap())
            .count(),
        1
    );
    if let Ok(destination) = std::env::var("GRAPHARCH_CARD_REVIEW") {
        tokio::fs::create_dir_all(&destination).await?;
        let mut files = tokio::fs::read_dir(&publication.directory).await?;
        while let Some(file) = files.next_entry().await? {
            if file.file_type().await?.is_file() {
                tokio::fs::copy(
                    file.path(),
                    std::path::Path::new(&destination).join(file.file_name()),
                )
                .await?;
            }
        }
        tokio::fs::create_dir_all(
            std::path::Path::new(&destination).join("brand"),
        )
        .await?;
        let mut files =
            tokio::fs::read_dir(publication.directory.join("brand")).await?;
        while let Some(file) = files.next_entry().await? {
            tokio::fs::copy(
                file.path(),
                std::path::Path::new(&destination)
                    .join("brand")
                    .join(file.file_name()),
            )
            .await?;
        }
    }
    let styles =
        tokio::fs::read_to_string(publication.directory.join("style.css"))
            .await?;
    assert!(styles.contains(".project-section-features h2{text-align:center"));
    assert!(
        tokio::fs::read_to_string(
            publication.directory.join("brand/LUCIDE-LICENSE.txt")
        )
        .await?
        .contains("ISC License")
    );
    Ok(())
}

#[tokio::test]
async fn unknown_panel_heading_fails_instead_of_silently_styling_another_section()
-> Result<()> {
    let input = tempfile::tempdir()?;
    let settings = input.path().join("project.ttl");
    tokio::fs::write(&settings, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:p> a ga:Project; dct:title "Project"; ga:scope <urn:s>; ga:target <urn:t> .
<urn:s> a ga:Scope .
<urn:profile> a ga:Profile; ga:theme ga:EkgfProjectTheme .
<urn:t> a ga:Target; dct:title "Website"; ga:scope <urn:s>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
"#).await?;
    tokio::fs::write(input.path().join("index.md"), "---\ntitle: Project\npermalink: /\ngrapharch:\n  subject: urn:home\n  panel_sections: [missing-heading]\n---\n# Project\n\n## Existing heading\n\nText.\n").await?;
    let collected =
        website::collect(input.path(), &settings, "urn:p", "urn:t", 2).await?;
    let output = tempfile::tempdir()?;
    let error = match Publisher::new(collected.dataset)
        .render(&collected.plan, output.path())
        .await
    {
        Err(error) => error,
        Ok(_) => panic!("unknown panel heading must fail publication"),
    };
    assert!(
        format!("{error:#}")
            .contains("missing-heading is not a level-two page heading")
    );
    Ok(())
}

#[tokio::test]
async fn invalid_feature_card_hints_fail_publication() -> Result<()> {
    for (hint, expected) in [
        ("hero: unknown", "unsupported hero presentation"),
        (
            "feature_sections: [missing]",
            "presentation section missing",
        ),
        (
            "resource_sections: [missing]",
            "presentation section missing",
        ),
        (
            "feature_sections: [benefits]\n  resource_sections: [benefits]",
            "conflicting presentations",
        ),
        (
            "card_icons:\n    benefit: {name: absent, tone: primary}",
            "unsupported card icon absent",
        ),
        (
            "card_icons:\n    benefit: {name: network, tone: absent}",
            "unsupported icon tone absent",
        ),
        (
            "card_icons:\n    missing: {name: network, tone: primary}",
            "card icon missing",
        ),
    ] {
        let input = tempfile::tempdir()?;
        let settings = input.path().join("project.ttl");
        tokio::fs::write(&settings, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:p> a ga:Project; dct:title "Project"; ga:scope <urn:s>; ga:target <urn:t> .
<urn:s> a ga:Scope .
<urn:profile> a ga:Profile; ga:theme ga:EkgfProjectTheme .
<urn:t> a ga:Target; dct:title "Website"; ga:scope <urn:s>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
"#).await?;
        tokio::fs::write(
            input.path().join("index.md"),
            format!(
                "---\ntitle: Project\npermalink: /\ngrapharch:\n  subject: \
                 urn:home\n  {hint}\n---\n# Project\n\n## Benefits\n\n### \
                 Benefit\n\nText.\n"
            ),
        )
        .await?;
        let collected =
            website::collect(input.path(), &settings, "urn:p", "urn:t", 2)
                .await?;
        let output = tempfile::tempdir()?;
        let error = match Publisher::new(collected.dataset)
            .render(&collected.plan, output.path())
            .await
        {
            Err(error) => error,
            Ok(_) => panic!("invalid hint must fail"),
        };
        assert!(
            format!("{error:#}").contains(expected),
            "{error:#}"
        );
    }
    Ok(())
}
