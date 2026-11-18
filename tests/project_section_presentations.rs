//! Generic project-section presentations selected by RDF hints: callout
//! bands, term lists and compact step sections, and the profile's opt-out of
//! the generator credit line.
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
<urn:github> dct:title "GitHub"; ga:url "https://github.com/example/example"; ga:order 1 .
<urn:target> a ga:Target; dct:title "Site"; ga:scope <urn:scope>; ga:profile <urn:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
"#;

const PAGE: &str = r#"---
title: Plan
permalink: /
grapharch:
  subject: urn:plan
  callout_sections: [join-us]
  term_sections: [standards]
  compact_sections: [one, two]
  card_icons:
    dcat: {name: layers, tone: primary}
    rdf: {name: layers, tone: accent}
---
# Plan

The lead.

## One

First step.

[Examples](https://example.org/examples/) [Local](#two)

## Two

Second step.

## Standards

The stack.

### [DCAT](https://example.org/dcat/)

**Data Catalog Vocabulary**

Catalogues.

### RDF

**Resource Description Framework**

Graphs.

## Join us

Come along.

[Specification](#one) [Repository](https://example.org/repo/)
"#;

async fn publish(definitions_text: &str, page: &str) -> Result<String> {
    let input = tempfile::tempdir()?;
    let definitions = input.path().join("project.ttl");
    tokio::fs::write(&definitions, definitions_text).await?;
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
async fn hints_select_callouts_term_lists_and_compact_sections() -> Result<()> {
    let html = publish(DEFINITIONS, PAGE).await?;
    // A callout band keeps its heading, prose and action row.
    assert_eq!(
        count(
            &html,
            "section.project-section-callout h2#join-us"
        ),
        1
    );
    assert_eq!(
        count(
            &html,
            ".project-section-callout p.project-section-actions a"
        ),
        2
    );
    // A term list: one row per level-three heading, its strong paragraph as
    // the expansion beside the term and the rest as the definition.
    assert_eq!(
        count(&html, ".project-section-terms .project-term"),
        2
    );
    assert_eq!(
        count(&html, ".project-section-terms .project-card"),
        0
    );
    assert_eq!(
        count(
            &html,
            ".project-term-primary > .project-term-name h3#dcat"
        ),
        1
    );
    assert_eq!(
        count(
            &html,
            ".project-term-accent > .project-term-name \
             p.project-term-expansion"
        ),
        1
    );
    assert_eq!(count(&html, ".project-term-definition > p"), 2);
    assert_eq!(
        count(&html, ".project-term .project-card-icon"),
        0
    );
    // Compact sections stay ordinary sections, marked for one rhythm.
    assert_eq!(count(&html, "section.project-section-compact"), 2);
    // Section actions mark each link internal or external.
    assert_eq!(
        count(
            &html,
            "#one ~ p.project-section-actions a svg, .project-section-compact \
             p.project-section-actions a svg"
        ),
        2
    );
    // The generator credit is on unless the profile turns it off.
    assert_eq!(count(&html, ".site-footer-credit"), 1);
    Ok(())
}

#[tokio::test]
async fn a_profile_can_omit_the_generator_credit() -> Result<()> {
    let html = publish(
        &DEFINITIONS.replace(
            "ga:siteFooter <urn:footer> .",
            "ga:siteFooter <urn:footer>; ga:generatorCredit false .",
        ),
        PAGE,
    )
    .await?;
    assert_eq!(count(&html, ".site-footer-credit"), 0);
    assert!(!html.contains("Generated by"));
    assert_eq!(count(&html, ".site-footer"), 1);
    Ok(())
}

#[tokio::test]
async fn a_section_selects_one_presentation() -> Result<()> {
    let error = publish(
        DEFINITIONS,
        &PAGE.replace(
            "term_sections: [standards]",
            "term_sections: [join-us]",
        ),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}")
            .contains("section join-us has conflicting presentations"),
        "{error:#}"
    );
    Ok(())
}
