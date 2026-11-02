use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

#[tokio::test]
async fn website_metadata_survives_kg_restore_and_indexes_visible_pages()
-> Result<()> {
    let input = tempfile::tempdir()?;
    let settings = input.path().join("project.ttl");
    tokio::fs::write(&settings, r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Example project"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope .
<urn:profile> a ga:Profile; ga:style "ekgf-project" .
<urn:target> a ga:Target; dct:title "Project website"; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site"; ga:thisVersion <https://example.org/project/> .
"#).await?;
    tokio::fs::write(
        input.path().join("index.md"),
        "---\ntitle: Home\ndescription: A project & its purpose.\npermalink: \
         /\ngrapharch:\n  subject: urn:home\n---\n# Example project\n\nAn \
         introduction.\n\n[Get started](guide.md)\n\n## Benefits\n\n### First \
         benefit\n\nA source-owned explanation.\n\n### Second \
         benefit\n\nAnother explanation.\n\n## Next steps\n\n[Read the \
         guide](guide.md)\n",
    )
    .await?;
    tokio::fs::write(
        input.path().join("guide.md"),
        "---\ntitle: Guide\ndescription: Practical guidance.\npermalink: \
         /guide/\ngrapharch:\n  subject: urn:guide\n---\n[Home](index.md)\n",
    )
    .await?;
    tokio::fs::write(
        input.path().join("hidden.md"),
        "---\ntitle: Hidden\npermalink: /hidden/\nnav_exclude: \
         true\ngrapharch:\n  subject: urn:hidden\n---\n# Hidden\n",
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
    assert!(html.contains(
        "name=\"description\" content=\"A project &amp; its purpose.\""
    ));
    assert!(
        html.contains(
            "rel=\"canonical\" href=\"https://example.org/project/\""
        )
    );
    assert!(
        html.contains("property=\"og:site_name\" content=\"Example project\"")
    );
    let guide = tokio::fs::read_to_string(
        publication.directory.join("guide/index.html"),
    )
    .await?;
    assert!(guide.contains(
        "property=\"og:url\" content=\"https://example.org/project/guide/\""
    ));
    let sitemap =
        tokio::fs::read_to_string(publication.directory.join("sitemap.xml"))
            .await?;
    assert!(sitemap.contains("<loc>https://example.org/project/guide/</loc>"));
    assert!(!sitemap.contains("provenance"));
    assert!(!sitemap.contains("/hidden/"));
    let hidden = tokio::fs::read_to_string(
        publication.directory.join("hidden/index.html"),
    )
    .await?;
    assert!(hidden.contains("name=\"robots\" content=\"noindex,follow\""));
    assert!(!sitemap.contains("lastmod"));
    let robots =
        tokio::fs::read_to_string(publication.directory.join("robots.txt"))
            .await?;
    assert!(
        robots.contains("Sitemap: https://example.org/project/sitemap.xml")
    );
    let report = tokio::fs::read_to_string(
        publication.directory.join("provenance.html"),
    )
    .await?;
    assert!(report.contains("name=\"robots\" content=\"noindex,follow\""));
    Ok(())
}

#[tokio::test]
async fn malformed_canonical_bases_fail_publication() -> Result<()> {
    for base in [
        "urn:website",
        "https://example.org/no-slash",
        "https://user@example.org/",
        "https://example.org/?query=yes",
    ] {
        let input = tempfile::tempdir()?;
        let settings = input.path().join("project.ttl");
        tokio::fs::write(&settings, format!(r#"@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Example"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope . <urn:profile> a ga:Profile .
<urn:target> a ga:Target; dct:title "Website"; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer "website"; ga:destination "site"; ga:thisVersion <{base}> ."#)).await?;
        tokio::fs::write(
            input.path().join("index.md"),
            "---\ntitle: Home\npermalink: /\ngrapharch:\n  subject: \
             urn:home\n---\n# Home\n",
        )
        .await?;
        let collected = website::collect(
            input.path(),
            &settings,
            "urn:project",
            "urn:target",
            1,
        )
        .await?;
        let output = tempfile::tempdir()?;
        assert!(
            Publisher::new(collected.dataset)
                .render(&collected.plan, output.path())
                .await
                .is_err(),
            "invalid canonical base {base}"
        );
    }
    Ok(())
}
