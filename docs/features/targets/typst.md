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

## Choose or define a theme

A profile selects its house style with `ga:theme`. Themes are RDF resources
identified by IRIs. GraphArch ships `ga:ReaderTheme` (the default for PDF and
websites), `ga:SpecificationTheme` (the default for specifications; it follows
the publisher's accent colour), `ga:OverviewTheme`, `ga:EkgfTheme` and
`ga:EkgfProjectTheme` in its [theme catalogue](../../other/ontology/themes.ttl).
The EKGF themes select the EKGF website identity; for PDF they use the reader
tokens.

A consumer theme refines a catalogue theme with `ga:baseTheme` and declares
only the tokens it changes:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .

<urn:example:house> a ga:Theme ;
    ga:baseTheme ga:SpecificationTheme ;
    ga:paper "us-letter" ; ga:marginLeft "0.79in" ;
    ga:bodyFont "Liberation Serif" ; ga:justify false ; ga:hyphenate false ;
    ga:headingNumberGap "0.5in" ;
    ga:tableHeaderFill "#4e80bc" ; ga:tableHeaderText "#ffffff" ; ga:tableGrid true ;
    ga:footerShowsText true ; ga:outerPageNumber true ; ga:pageOfTotal false ;
    ga:frontNumbering "i" ; ga:bodyNumbering "1" ;
    ga:tocTitle "Table of Contents" ; ga:tocTitleAlign "center" ; ga:contentsDepth 2 ;
    ga:annexLabel "Annex {letter}: {title}" ; ga:annexNormativity true ;
    ga:annexRectoStart true ; ga:blankPageText "This page intentionally left blank." .

<urn:example:pdf-profile> a ga:Profile ; ga:theme <urn:example:house> .
```

Tokens cover page size and margins; colours; body, heading, code, caption,
footer, contents and notice fonts and sizes; justification and hyphenation;
heading numbering, a fixed number column and chapter page breaks; table header
fill, text and grid; link colour and underlining; the footer text
(`ga:footerText` on the target, else the title) and a page number on the outer
edge; front and body page numbering; the contents title, alignment and depth;
and annex labels, a normativity line from the placement's `ga:normativity`, and
recto starts. [The vocabulary](../../other/ontology/publication.ttl) defines each
token and its value syntax. GraphArch validates every value and maps it to a
Typst value; no supplied text is evaluated. An invalid value, an unknown token,
a missing token or a theme that does not refine a catalogue theme fails the
publication.

The cover is an ordered list of `ga:coverBlock` entries. Each places one
element: space, the publisher, its logo (`ga:logo`, PNG or SVG), the
publication date in a chosen format, the title, the subtitle, a version line,
a rule, a bar, the target's ordered metadata rows with their definition labels,
the specification summary, or fixed text. The values come from the target, so
a theme supplies no document number, status or legal wording.

The bundled fonts are Liberation Sans, Serif and Mono, metric-compatible
substitutes for Arial, Times New Roman and Courier New, plus Typst's embedded
fonts. They are openly licensed; they are not the proprietary originals.

## The OMG Commons edition

The [Commons showcase](https://github.com/EKGF/grapharch/tree/main/examples/omg-commons)
declares OMG's house style as a consumer theme, OMG's notices, preface and
copyright holders, the OMG logo, and the Commons 1.3 release metadata. Its
profile sets `ga:w3cFrontMatter false`, omits the family overview, coverage,
quality and diagram-note material from the PDF, and nests the ontology chapters
under an authored clause. Its cover states that it is a GraphArch rendering of
Commons 1.3. The design references the
[Commons 1.3 PDF](https://www.omg.org/spec/Commons/1.3/PDF),
[OMG core templates at revision 7e9dbce](https://github.com/ObjectManagementGroup/mdsa-omg-core/tree/7e9dbcef362b47a1c362f93b7ee2b840fb694997),
and [OMG specification template at revision 9801480](https://github.com/ObjectManagementGroup/mdsa-omg-spec/tree/98014805e3ea8ec7d865f91453dddaf89fb4222a).

## Publish an audience overview

Select `ga:theme ga:OverviewTheme` on a PDF profile for an editorial introduction:
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
