---
title: Publishing and verifying showcases
---

# Publishing and verifying showcases

The previews below display the actual generated HTML. All publication artifacts
are retained in the KG and can be rendered again after their source files are
removed. The examples also serve as regression and live integration tests.

Every declared showcase is attempted, even when another case fails. Unexpected
failures block a new publication. Verification retains a structured per-case
report, KG diagnostics, and collected case graphs where available; declared
expected-error examples are checked against their expected diagnostics.

See [Examples and showcases](usage/examples.md) for runnable commands, fixture
coverage, and the limits of each demonstration.

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
and download remains available. Each showcase introduction retains all named PDF editions and links to the other publication outputs.
