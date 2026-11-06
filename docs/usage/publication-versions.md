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
specification was authored. Optional `ga:order` values place canonical releases
and development versions before previews; versions without an explicit order
follow alphabetically by branch. The page shows a readable UTC observation time
below the version-list heading.

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
    ga:advertised true ; ga:order 1 .

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

For GitHub repositories deployed on Vercel, configure the repository publication
collector to capture open pull requests and ready deployments during collection:

```turtle
<urn:example:project> ga:collector <urn:example:versions-collector> .
<urn:example:versions-collector> a ga:Collector ;
    ga:kind "repository-publications" ;
    ga:source "https://github.com/example/project" ;
    ga:subject <urn:example:inventory> ;
    ga:publicationPath "project/spec/" ;
    ga:unlistedBranchPrefix "dependabot/" .
```

The build requires `GITHUB_TOKEN` with repository pull-request read access and
`VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_PROJECT_ID` with deployment read access.
Credentials remain in request headers. Source-defined canonical versions such
as `main` and `develop` retain their configured titles and URLs. Other branches
use the newest ready deployment's immutable URL; only open pull requests from
the same repository are advertised. Closed, merged and explicitly unlisted
branches remain in the routing manifest while their deployments exist.

Each API is paginated with a 1,000-record budget and 2 MiB per response page.
Authentication failures, malformed records, exhausted budgets and colliding
branch slugs fail collection. A failed refresh must leave the previous complete
publication deployed. This is a build-time snapshot: schedule refreshes and show
the recorded observation time instead of promising live deployment discovery.


For an audience-facing PDF that should omit its visible audit appendix, set
`ga:showProvenance false` on that target's publication profile. The default is
`true`. This controls the PDF presentation only: collected provenance and the
publication manifest remain available for audit, and other profiles retain their
appendices. Keep human acknowledgments in authored content; recent Git activity
is not a substitute for collective authorship credits.

For an assembled site, a version may also declare an explicit local
`ga:route "/project/spec/main/"`. The version page uses that host-root path,
while `dcat:accessURL` retains the canonical public address. The route survives
KG export/restoration and appears in `publication-versions.json`. Routes must
use safe nonempty path segments without traversal. Packaging requires a matching
assembled `index.html` inside its mount; a missing selected publication fails
instead of redirecting to an older remote deployment.
