---
title: Legal Preamble
parent: Collectors
nav_order: 7
permalink: /collectors/legal-preambles/
grapharch:
  subject: urn:grapharch:docs:collectors:legal-preambles
---

# Legal Preamble Collector

The legal-preamble collector reads one declarative HTML disclosure and attaches
its retained legal fragment to explicitly selected publication targets. It does
not run template code or JavaScript. This is distinct from collecting an entire
published specification.

## What it documents

It supports [legal preambles](../documentable-things/legal-preambles.md)
for specifications and other formal publications.
The specification renderer can show it as an expandable disclosure, while the PDF
presents the retained legal text in its own front-matter styling.

## Supply one legal disclosure

```html
<details class="preamble-section">
  <summary>Preamble</summary>
  <h2>Use of specification</h2>
  <p>The specification is provided under the stated terms.</p>
  <h2>Licenses</h2>
  <p>Include the source-approved license text here.</p>
</details>
```

The adapter requires the unique `details.preamble-section` element. Preserve the
authoritative wording; this example illustrates markup, not legal advice or
replacement license language. Its base URL resolves supported source links.

In an [exact-revision build](../usage/publication-pipeline.md#exact-revision-builds),
a collector use with `ga:HtmlPreambleCollector` names the template's
`ga:sourcePath` and the `ga:fragment` it contributes. The collector never
chooses targets: each target that shows the notices references that fragment
with `ga:preamble`, and a profile can require it with `ga:requiredMetadata
ga:preamble`:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix ex: <urn:example:> .

ex:project ga:collector ex:legal-source .
ex:legal-source a ga:CollectorUse ; ga:collectorType ga:HtmlPreambleCollector ;
    ga:source "respec/template.html" ; ga:sourcePath "respec/template.html" ;
    ga:fragment ex:legal-preamble .
ex:specification ga:preamble ex:legal-preamble .
ex:profile ga:requiredMetadata ga:preamble .
```

There is no `publication collect --kind preamble` CLI switch. Library
integrations call `HtmlPreambleFile::new` with the source path, base URL and
fragment IRI, or `HtmlPreambleFile::from_git` for a pinned repository blob.
Arbitrary repository templates are not automatically treated as legal
preambles. See [the architecture](../other/architecture/README.md) for the
collector-to-KG handoff and [publication profiles](../usage/specification-renderer.md)
for target metadata.
