---
title: Websites and PDFs
parent: Features
nav_order: 4
permalink: /features/targets/
grapharch:
  subject: urn:grapharch:docs:topic:features:targets
---

# Publish for different readers

GraphArch publishes final HTML, CSS, and PDF artifacts directly from the
documentation graph. A project can provide an authoritative reference,
explanatory companion site, and review document with different arrangements of
shared content.

## Publication styles

| Style | Useful when readers need | Output |
| --- | --- | --- |
| Companion website | Guides, stories, examples, and topic navigation | Static pages with breadcrumbs, page contents, and previous/next links. |
| Specification | A familiar standards document with publication metadata and numbered sections | Standards-style HTML with a paired PDF download. |
| Paginated specification | A large reference that is impractical as one HTML document | Smaller linked specification pages and shared navigation/search. |
| Bookshelf | A large ontology family organised into declared modules | A catalogue of module books, each with its own specification and PDF. |
| Independent PDF | An explicitly arranged review or approval document | A PDF and its editable Typst source. |
| Single-document HTML | A compact document with a contents list | One HTML document and CSS. |

The [HTML feature guide](html.md) describes static hosting and navigation. The
[PDF feature guide](typst.md) describes document presentation.

## One content model, independent publications

Each target has its own scope, order, renderer, and output directory. A PDF need
not reproduce a website's page boundaries. A companion article need not appear
in the specification. Deliberate reuse keeps shared definitions connected to the
same source revision while allowing a different reading order.

The specification style supports configurable publisher branding, status,
version links, editors, authors, legal notices, references, and appendices.
Required-content profiles can block a release until its required sections and
publisher-assigned metadata exist. A standards-like appearance does not imply
approval by the named authority.

## Explore the available layouts

- [Specification renderer](../../usage/specification-renderer.md)
- [Module books and bookshelves](../../usage/bookshelves.md)
- [Specification, companion site, and PDF](../../usage/publication-families.md)
- [Showcase gallery](../../showcases.md)
