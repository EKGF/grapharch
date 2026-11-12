---
title: Published specifications
parent: Documentable Things
nav_order: 7
permalink: /documentable-things/published-specifications/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:specifications
---

# Published specifications

## Example: preserve a published section and example

```html
<section id="introduction">
  <h2>Introduction</h2>
  <p>This document describes the exchange format.</p>
  <aside class="example" id="example-triple">
    <div class="marker">Example 1: A triple</div>
    <pre>&lt;urn:customer:1&gt; &lt;urn:name&gt; "Ada" .</pre>
  </aside>
</section>
```

This illustrates the static HTML structures in a published specification, not
a complete input document. GraphArch retains supported sections, anchors, and
examples from the full published snapshot, then publishes a new specification
and PDF from the collected graph.

Compare the [complete N-Triples input snapshot](https://github.com/EKGF/grapharch/blob/main/examples/w3c-rdf12-n-triples/source.html)
with the [generated specification](https://ekgf.github.io/grapharch/showcases/artifacts/rdf12/specification/index.html)
and [PDF](https://ekgf.github.io/grapharch/showcases/artifacts/rdf12/specification/document.pdf).
Look at **Introduction** and the numbered examples in **N-Triples Language**.

## Supporting collectors

The [Published W3C Specification Collector](../collectors/published-specifications.md)
reads supported published HTML. The [Legal Preamble Collector](../collectors/legal-preambles.md)
separately reads a supported legal disclosure, such as DPROD's preamble. Neither
requires executing the original website generator.

## Recognised published content

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
[front-matter notices](../usage/specification-renderer.md#front-matter-notices)
for the fragment and target relationship.

## Publication authority is independent

A collected specification's content does not require every output to have the
source website's branding. The publication target and authority metadata select
the generated specification's identity and presentation. See the
[specification workflow](../usage/specification-renderer.md) and
[showcase gallery](../showcases.md) for current examples.
