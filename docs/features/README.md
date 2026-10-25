---
title: Features
layout: default
nav_order: 3
permalink: /features/
grapharch:
  subject: urn:grapharch:docs:topic:features
---

# Grapharch feature scope

This catalog describes the intended product scope. See the
[implementation status](../other/status.md) for working capabilities and remaining
delivery work.

## General Features

- [**Ontology Precision**](./general/ontology-precision.md):
  GraphArch emphasizes governance and precision in documenting ontologies,
  schemas, and their relationships. It aims to deliver librarian-level
  accuracy of definitions, domain ownership, and control for enterprise data management.
- [**Business-Oriented Profiling**](./general/business-oriented-profiling.md):
  GraphArch offers capabilities for graph database endpoint analysis that
  can be particularly useful for business users to understand and describe
  their data in their own terms.
- [**Flexible Output Formats**](./general/flexible-output-formats.md):
  Documentation can be generated as user-friendly websites or professionally
  formatted PDF reports.
- [**Onboard Graph Engine**](./general/onboard-graph-engine.md):
  The tool includes [OxiGraph](https://github.com/oxigraph/oxigraph),
  a graph database that can be used to generate data for further documentation,
  integrating both input options seamlessly.


## Sources & Formats

See [Formats](./formats/README.md)

## Targets

Grapharch renders final publication artifacts from the documentation KG:

- [HTML and CSS](./targets/html.md): static documentation websites and reader
  layouts generated directly by Rust.
- [Specifications](../usage/specification-renderer.md): standards-style HTML
  with a paired PDF from the same pinned plan.
- [PDF via Typst](./targets/typst.md): document-oriented output with pagination,
  a table of contents, references, and appendices.

Each target has its own scope and arrangement while reusing logical fragments
from the same graph revision.
