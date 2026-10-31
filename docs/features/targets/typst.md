---
title: PDF documents
parent: Websites and PDFs
nav_order: 2
permalink: /features/targets/typst/
grapharch:
  subject: urn:grapharch:docs:topic:features:targets:typst
---

# Publish a document, not a screenshot of a website

GraphArch generates a typeset PDF and editable Typst source from the selected
content in the documentation KG. No separate typesetting installation is
required. A PDF can share a specification's content or use an independently
planned reading order for review.

## Document structure and presentation

The specification presentation follows the publisher's accent colour and
includes publication metadata. The reader presentation uses the companion
site's slate and teal palette. Documents support a cover, linked contents,
running headings, page numbers, tables with repeated headers, code, retained
images and diagrams, Markdown alerts, references, and appendices.

Legal notices can occupy their own styled front-matter pages. A publisher
preface is distinct from the legal preamble and ordinary numbered chapters.
Quality findings are grouped by warning type in a separate numbered appendix.
Supplied content and release metadata remain source-owned; a layout is not
proof of standards approval.

## Match the publication to its size

A small reference can have one PDF. A large ontology family can use
[bookshelf publication](../../usage/bookshelves.md) to create module books with
separate PDFs and chapter navigation. Unchanged book PDFs can be reused after
their input fingerprints and checksums are verified.

PDF compilation can be substantially slower than HTML generation for large or
complex documents. Book boundaries and contents depth help keep the result
usable; they do not make a very large ontology small.

Dedicated taxonomist-oriented print layouts are not currently supplied. SKOS
content can appear in the existing resource-reference documents; a specialist
hierarchical or alphabetical taxonomy edition requires further renderer work.

See [publication families](../../usage/publication-families.md) to separate a
specification, companion site, and review PDF.
