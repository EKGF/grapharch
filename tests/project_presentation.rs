//! Generic project-page presentations: badges, lead resource lists, timelines,
//! closing notes and footer link icons, all selected by RDF hints.
use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
};

const DEFINITIONS: &str = r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Example"; ga:scope <urn:scope>; ga:target <urn:target> .
<urn:scope> a ga:Scope .
<urn:profile> a ga:Profile; ga:theme ga:EkgfProjectTheme; ga:siteFooter <urn:footer> .
<urn:footer> a ga:SiteFooter; ga:footerGroup <urn:links> .
<urn:links> dct:title "Links"; ga:order 1; ga:footerLink <urn:github> .
<urn:github> dct:title "GitHub"; ga:url "https://github.com/example/example"; ga:order 1; ga:iconName "github" .
<urn:target> a ga:Target; dct:title "Site"; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
"#;

const PAGE: &str = r#"---
title: Plan
permalink: /
grapharch:
  subject: urn:plan
  resource_sections: [plan]
  timeline_sections: [timeline]
  timeline_statuses:
    Done: {name: circle-check, tone: primary}
    Planned: {name: circle, tone: muted}
  card_icons:
    stable: {name: archive, tone: accent, badge: Frozen}
    draft: {name: clock, tone: muted}
---
Roadmap

# Plan

The lead.

### [Stable](https://example.org/stable/)

The frozen one.

*`/stable/` · branch `main`*

### Draft

No link.

## Timeline

1. Done — 2024

   **Shipped**

   It shipped.
2. Planned — Later

   **Next**

   Comes next.

## Notes

### A card

Card text.

---

> A closing note.
"#;

async fn publish(page: &str) -> Result<String> {
    let input = tempfile::tempdir()?;
    let definitions = input.path().join("project.ttl");
    tokio::fs::write(&definitions, DEFINITIONS).await?;
    tokio::fs::write(input.path().join("index.md"), page).await?;
    let collected = website::collect(
        input.path(),
        &definitions,
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
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&collected.plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    Ok(
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?,
    )
}

fn count(html: &str, selector: &str) -> usize {
    scraper::Html::parse_document(html)
        .select(&scraper::Selector::parse(selector).unwrap())
        .count()
}

#[tokio::test]
async fn hints_select_badges_lead_lists_timelines_notes_and_icons() -> Result<()>
{
    let html = publish(PAGE).await?;
    // The page title's anchor presents the lead cards as a resource list.
    assert_eq!(
        count(&html, ".project-section-resources .project-card"),
        2
    );
    assert_eq!(
        count(
            &html,
            ".project-card-linked .project-card-title h3#stable"
        ),
        1
    );
    assert_eq!(
        count(
            &html,
            ".project-card-title .project-badge.project-badge-accent"
        ),
        1
    );
    assert!(html.contains(">Frozen</span>"));
    assert_eq!(
        count(
            &html,
            ".project-card-linked > .project-card-arrow"
        ),
        1
    );
    assert_eq!(count(&html, ".project-card-icon-muted"), 2);
    assert_eq!(count(&html, "p.project-card-meta code"), 2);
    // The timeline rail.
    assert_eq!(
        count(
            &html,
            ".project-section-timeline ol.project-timeline"
        ),
        1
    );
    assert_eq!(count(&html, ".project-timeline-item"), 2);
    assert_eq!(
        count(
            &html,
            ".project-timeline-meta .project-badge-primary"
        ),
        1
    );
    assert_eq!(count(&html, "p.project-timeline-title"), 2);
    assert!(
        html.contains("<span class=\"project-timeline-date\">Later</span>")
    );
    // A thematic break returns closing prose to the section.
    assert_eq!(
        count(
            &html,
            ".project-section > .project-container > blockquote"
        ),
        1
    );
    assert_eq!(count(&html, ".project-card blockquote"), 0);
    assert_eq!(
        count(&html, ".site-footer .site-footer-icon svg"),
        1
    );
    Ok(())
}

#[tokio::test]
async fn an_undeclared_timeline_status_fails() -> Result<()> {
    let error =
        publish(&PAGE.replace("2. Planned — Later", "2. Someday — Later"))
            .await
            .unwrap_err();
    assert!(
        format!("{error:#}")
            .contains("timeline status \"Someday\" is not declared"),
        "{error:#}"
    );
    Ok(())
}

#[tokio::test]
async fn unknown_tones_fail() -> Result<()> {
    let error = publish(&PAGE.replace("tone: muted}\n---", "tone: loud}\n---"))
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("unsupported icon tone loud"),
        "{error:#}"
    );
    Ok(())
}
