---
title: Published specifications
parent: Documentable Things
nav_order: 7
permalink: /documentable-things/published-specifications/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:specifications
---

# Published specifications

A specialised collector reads a pinned snapshot of a published W3C
specification. It retains publication metadata, sections, headings, anchors,
links, examples, and supported static HTML content as documentation graph
content. The RDF 1.2 N-Triples showcase exercises this source path.

This collector reads published structure. It does not execute a ReSpec script
or claim compatibility with every ReSpec authoring extension. Unsupported
published markup fails explicitly. The
[ReSpec compatibility contract](../other/architecture/respec-compatibility.md)
records the migration reference and its acceptance criteria.

## Legal preamble fragments

A separate preamble collector recognises a supported legal-disclosure block in
a source template. DPROD supplies `details.preamble-section`; its complete legal
wording and source provenance are retained without executing the surrounding
scripts or template language.

A specification renderer can show that fragment as a disclosure, while the PDF
renderer gives it dedicated legal front matter. See
[legal preamble publication](../usage/specification-renderer.md#legal-preamble)
for the fragment and target relationship.

## Publication authority is independent

A collected specification's content does not require every output to have the
source website's branding. The publication target and authority metadata select
the generated specification's identity and presentation. See the
[specification workflow](../usage/specification-renderer.md) and
[showcase gallery](../showcases.md) for current examples.
