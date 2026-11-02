---
title: Publication versions
parent: Usage
nav_order: 16
permalink: /usage/publication-versions/
grapharch:
  subject: urn:grapharch:docs:usage:publication-versions
---
# Publication version listings

A publication inventory can generate a version-selection page and a matching
machine-readable routing manifest from the same pinned documentation graph.
The visible list can omit obsolete review branches while their existing URLs
remain routable.

Represent versions with `doap:Version`, a title, branch, identifier and
`dcat:accessURL`. `ga:advertised` explicitly selects which versions readers see.
The inventory's `dct:modified` records when it was observed, not when the
specification was authored.

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix doap: <http://usefulinc.com/ns/doap#> .
@prefix dcat: <http://www.w3.org/ns/dcat#> .

<urn:example:inventory> dct:modified "2026-11-02T15:00:00Z" ;
    dct:hasPart <urn:example:develop> .
<urn:example:develop> a doap:Version ;
    dct:title "Develop (latest)" ; dct:identifier "develop" ;
    doap:branch "develop" ;
    dcat:accessURL <https://example.org/spec/develop/> ;
    ga:advertised true .

<urn:example:versions-page> a ga:Documentor ;
    ga:project <urn:example:project> ; ga:kind "publication-inventory" ;
    ga:subject <urn:example:inventory> ; ga:fragment <urn:example:page> ;
    dct:title "Specification versions" ; ga:route "spec-versions/" ;
    ga:sourcePath "spec-versions.md" ; ga:navigationOrder 6 .
```

The normal website pipeline generates the page before resolving the publication
plan. Authored Markdown can link to `spec-versions.md` as it would any other page.
Rendering reads the retained inventory only; it never queries deployment services.

The website also contains `publication-versions.json`: an observation timestamp
and every known version's slug, branch, label, specification URL and advertised
flag. Static hosting configuration can preserve unadvertised preview links using
that manifest. Invalid metadata, ambiguous slugs and incomplete records fail the
build; they do not produce a deceptively shortened listing.
