---
title: Showcase gallery
nav_order: 6
permalink: /showcases/
grapharch:
  subject: urn:grapharch:docs:topic:showcase-gallery
  showcase_gallery: true
---

# Showcase gallery

Find an example for the sources you have and the publication you want to create.
Filter by source format, capability, use case, or available output. Every preview
is a real GraphArch publication: open its HTML, choose a PDF edition, or download
the retained documentation graph.

The previews below display the actual generated HTML. All publication artifacts
are retained in the KG and can be rendered again after their source files are
removed. The examples also serve as regression and live integration tests.

Every declared showcase is attempted, even when another case fails. Unexpected
failures block a new publication. Verification retains a structured per-case
report, KG diagnostics, and collected case graphs where available; declared
expected-error examples are checked against their expected diagnostics.

See [Examples and showcases](usage/examples.md) for runnable commands, fixture
coverage, and the limits of each demonstration.

The [OMG Commons example](https://github.com/EKGF/grapharch/tree/main/examples/omg-commons) collects the published
ontology library directly from OMG's RDF URLs. Its library overview links to an
individual specification page for each ontology, with a combined PDF and exported
KG. Unlike the Git repository examples, its source revisions are identified by
URL and content digest.

The **Multilingual code examples** showcase demonstrates Python, Bash, JavaScript,
TypeScript, JSON, JSON-LD and SPARQL. Its HTML and PDF use the same offline syntax
colours. See [Code examples and syntax colours](usage/code-highlighting.md) for
fence names and practical Markdown examples.

The DPROD showcase publishes two editions from the same pinned repository
revision and technical chapters: the
[EKGF community PDF](https://ekgf.github.io/grapharch/showcases/artifacts/dprod/pdf/ekgf/document.pdf)
and the
[OMG submission-preview PDF](https://ekgf.github.io/grapharch/showcases/artifacts/dprod/pdf/omg/document.pdf).
Their publication profiles select different front matter and publisher branding.
The submission preview does not claim OMG approval or an assigned OMG document
number.

## Describing a showcase

The source registry, `examples/showcases.ttl`, owns each example's title,
description, original source and credits. Repeated `ga:showcaseInput`,
`ga:showcaseCapability` and `ga:showcaseUseCase` labels describe what the example
actually demonstrates. `dct:source` links to its public source and `dct:rights`
provides concise attribution. Labels are limited to 64 bytes and 16 per category.
Available output filters come from the retained HTML, PDF and KG artifacts,
not from claims in the registry. PDF is optional: an HTML-only example still
provides its website and retained KG, without a fabricated PDF download.

Search and filters work entirely in the browser and keep their state in the URL,
including under a GitHub Pages project path. Without JavaScript, every showcase
and download remains available. Every card retains all named PDF editions and
links to the other publication outputs.
