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

## Tagged, accessible PDF

Every PDF, including each bookshelf book, is a tagged PDF: it carries a logical
structure tree for headings, paragraphs, lists, tables, figures and links, a
document language, and bookmarks that repeat the heading numbers. Images and
diagrams carry alternative text from their retained titles.

Document metadata comes from the pinned target definition, never from the build
machine. The title is the document or book title. The authors are the target's
`ga:author` people, or else the publishing authority. Repeat `ga:keyword` on the
target for keywords; each value is one `xsd:string`. The creation date is the
target's `ga:publicationDate`; an undated target embeds no date. The same pinned
plan therefore produces byte-identical PDFs.

To require [PDF/UA-1](https://www.iso.org/standard/64599.html), add
`ga:pdfStandard ga:PdfUa1` to the PDF profile:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .

<urn:example:pdf-profile> a ga:Profile ;
    ga:pdfStandard ga:PdfUa1 .
```

Typst checks the requirement while exporting each PDF. It requires a title,
alternative text for images, and consecutive heading levels, and makes viewers
display the document title. A violation fails the publication; it does not
produce a non-conforming PDF. Typst's automated checks cannot establish
everything PDF/UA asks of authors, such as a meaningful reading order or useful
alternative text, so review the result with an accessibility checker.

## Prepare an OMG-style edition

Set `ga:style "omg"` on a dedicated PDF profile. This selects a restrained
US Letter layout with one-inch margins, a 10-point Liberation Serif body, black Liberation Sans headings,
a plain ruled cover, Roman-numbered front matter and Arabic-numbered chapters.
The PDF and its editable source are still generated entirely with Typst.
Use a separate target and destination to retain an EKGF or reader edition from
the same pinned content revision.

The style does not supply an OMG logo, approval status, document identifier,
copyright notice, or licensing terms. Declare publication metadata and legal
preamble explicitly; an unofficial preview stays an unofficial preview.
The Commons gallery PDF demonstrates this profile without claiming to be the
published OMG specification.

The design references the [Commons 1.3 PDF](https://www.omg.org/spec/Commons/1.3/PDF),
[OMG core templates at revision 7e9dbce](https://github.com/ObjectManagementGroup/mdsa-omg-core/tree/7e9dbcef362b47a1c362f93b7ee2b840fb694997),
and [OMG specification template at revision 9801480](https://github.com/ObjectManagementGroup/mdsa-omg-spec/tree/98014805e3ea8ec7d865f91453dddaf89fb4222a).
It translates their presentation conventions; it does not execute LaTeX or copy
sample legal assertions. The templates specify US Letter; the inspected Commons
PDF has a narrower 595.56 by 792 point media box, so this is not a pixel-identical
reproduction of that Word-generated publication. The embedded Liberation fonts
are openly licensed, metric-compatible substitutes for its Times New Roman and
Arial fonts; they are not the proprietary originals. Font files, hashes, and
license notices are retained with the renderer sources.

## Publish an audience overview

Select `ga:style "overview"` on a PDF profile for an editorial introduction:
a prominent cover, indigo accent, orange cover marker, larger chapter openings,
and a more relaxed reading measure. Choose a separate scope and placements for
purpose, concepts, adoption, and selected examples. The same sources can supply
a website and a guide without putting the full technical reference in both.

An overview is a community explanation, not an approval or release declaration.
Its provenance section still records the source revision and generator.
An independently scoped guide can set `ga:qualityScope "selected"` on its profile
to report findings for the resources it selects. The default, `"all"`, retains
project-wide findings. Original diagnostics stay in the documentation KG.

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
