---
title: Renderers
nav_order: 6
permalink: /renderers/
grapharch:
  subject: urn:grapharch:docs:renderers:readme
---

# Renderers

A renderer turns a resolved publication plan into reader-facing artifacts.
[Collectors](../collectors/README.md) retain sources in the documentation knowledge
graph; documentation generators interpret those sources. Renderers consume the
selected content from a pinned graph revision, without rereading the original
source files.

## Publication renderers

| Renderer | Useful for | Output |
|---|---|---|
| [Website](website.md) | A static companion website for guides, explanations, and reference pages. | HTML pages with topic navigation, search, retained assets, and provenance. |
| [Specification](specification.md) | A single standards-style reference with publication metadata and numbered sections. | Specification HTML, a matching PDF, and editable Typst source. |
| [Paginated specification](specification-pages.md) | A large reference divided into linked HTML pages. | An HTML library landing page and linked sections with shared navigation and search. This mode does not produce a combined PDF; select a separate PDF target when needed. |
| [Bookshelf](bookshelf.md) | An ontology family arranged into declared module books. | A catalogue of books with individual HTML specifications and PDFs. Book boundaries come from the publication model, not guessed file groupings. |
| [PDF](pdf.md) | A review or approval document with its own reading order. | A PDF compiled by the embedded Typst engine, with editable Typst source. An external Typst executable is not required. |
| [Single-document HTML](html.md) | A compact standalone document with a contents list. | One HTML document, its stylesheet, retained illustrations, and provenance. Use the website renderer for a multi-page companion site. |

## Renderer, style, and format

`ga:renderer` links a target to a shared renderer definition IRI from the
[renderer catalogue](../other/ontology/renderers.ttl). Multiple targets can refer
to the same definition while selecting different scopes, profiles, and output
directories. Literal dispatch names are not accepted. A profile controls supported styles
and presentation hints. HTML and PDF are output formats: several renderers can
produce them. An OMG-style specification does not itself imply OMG approval.

## Fragment renderers

Publication renderers produce complete outputs. Fragment renderers draw embedded
parts of those outputs. Both have RDF identities under `ga:Renderer`, with
`ga:PublicationRenderer` and `ga:FragmentRenderer` distinguishing their roles.

`ga:OwlClassDiagramRenderer` is the shared UML diagram component. It draws retained
semantic projections as SVG for HTML and Typst/PDF, with links adapted to the
containing publication. It is selected internally when a publication contains a
class diagram; it cannot be selected as a target's whole-publication renderer.
These identities describe existing compiled components, not runtime plugin loading.

A knowledge-graph export preserves collected content and publication plans for
later rendering; it is not an additional renderer.

See [websites and PDFs](../features/targets/README.md) for the shared capabilities
and [publication families](../usage/publication-families.md) for multiple targets
from one graph revision.

## Contribute a renderer

The catalogue can grow through reviewed pull requests. Follow the existing
[renderer extension contract](../features/extensibility.md) and
[contribution guide](../other/contribute/README.md). A renderer consumes pinned
publication plans, preserves source identities and provenance, and must be tested
from a restored graph without access to the original sources. These are compiled
GraphArch implementations, not a claim of runtime plugin loading.
