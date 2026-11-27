---
title: Migration notes
parent: Usage
nav_order: 17
permalink: /usage/migration/
grapharch:
  subject: urn:grapharch:docs:usage:migration
---
# Migration notes

Breaking changes are listed first in each GitHub release, collected from
commits marked with `!` or a `BREAKING CHANGE:` footer. This page explains what
to change for those that need more than one line. The version given is the
first release containing the change; a build that skips several releases
meets every change in between.

## `ga:renderer` takes a renderer IRI (0.2.8)

A target's `ga:renderer` is no longer a string such as `"website"`. It is the
IRI of a `ga:Renderer` from the renderer catalogue; string values fail
validation.

| Old value | Renderer IRI |
| --- | --- |
| `"html"` | `ga:HtmlRenderer` |
| `"website"` | `ga:WebsiteRenderer` |
| `"specification"` | `ga:SpecificationRenderer` |
| `"specification-pages"` | `ga:SpecificationPagesRenderer` |
| `"bookshelf"` | `ga:BookshelfRenderer` |
| `"typst"` | `ga:TypstRenderer` |

## OWL class diagrams need Java and PlantUML (0.2.0)

Documenting an ontology draws a PlantUML class diagram for every OWL class.
Collection and documentation therefore need Java and a `plantuml` launcher on
`PATH`; rendering an already documented KG needs neither. The GraphArch GitHub
Action installs both. Elsewhere, install them as described in
[Local UML rendering dependency](github-actions.md#local-uml-rendering-dependency).
There is currently no opt-out; an upcoming profile switch will let a
publication omit these diagrams.

## PDF builds need `--edition` and a publisher slug (0.2.10)

Every build that renders a PDF (`build-family`, `build-site`,
`build-revision`, `render` and the Action's `edition` input) requires
`--edition`. Pass `default` for the declared default branch, otherwise a branch
slug, `pr-<number>` or the release version.

A PDF target also needs a `ga:slug` on its `ga:publisher`, a `ga:documentType`
(for example `ga:SpecificationDocument`), and a `ga:slug` on the target or its
family. A missing declaration is an error.

## PDF file names follow the edition name (0.2.10)

A PDF target no longer writes `document.pdf` in its destination. It writes
`<authority>-<document-type>-<document-name>[-<edition>].pdf`, for example
`ekgf-spec-use-case-ontology.pdf` for the default edition and
`ekgf-spec-use-case-ontology-pr-23.pdf` for a preview. Update links, download
steps and `ga:pdfPath` values that name `document.pdf`. See
[PDF edition names](publication-families.md#pdf-edition-names).

## `ga:Authority` is replaced by `artgov:Authority` (0.2.0)

Publishing authorities use the imported Artifact Governance class
`<https://ekgf.org/ontology/artifact-governance#Authority>` directly. Change
`a ga:Authority` to `a artgov:Authority` with

```turtle
@prefix artgov: <https://ekgf.org/ontology/artifact-governance#> .
```

Its properties (`rdfs:label`, `ga:shortName`, `ga:url`, `ga:accentColor`, `ga:logo`,
and the `ga:slug` used in PDF names) are unchanged.
