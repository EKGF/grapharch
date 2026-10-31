---
title: Published W3C Specification Collector
parent: Collectors
nav_order: 6
permalink: /collectors/published-specifications/
grapharch:
  subject: urn:grapharch:docs:collectors:published-specifications
---

# Published W3C Specification Collector

This collector reads a saved published W3C HTML document with its original source
URL. It extracts supported document structure without executing ReSpec or the
page's JavaScript. It is not a universal HTML importer or a complete replacement
for every ReSpec extension.

## What it documents

It supports [published specification content](../documentable-things/published-specifications.md):
sections and heading identities, examples, supported references, and publication
metadata. The original specification URL establishes source identity and link
resolution. The current collector requires an HTTPS URL on `www.w3.org` without
a fragment.

## Collect the existing reference fixture

The repository includes a saved RDF 1.2 N-Triples source used by its showcase:

```sh
grapharch publication collect --store output/w3c-kg \
  --project urn:w3c:project --collector urn:w3c:n-triples \
  --kind w3c-specification \
  --source examples/w3c-rdf12-n-triples/source.html \
  --source-url https://www.w3.org/TR/2026/WD-rdf12-n-triples-20261023/
```

Run this from the GraphArch repository. For another supported specification,
supply its saved HTML and the matching publication URL. The command reads the
saved file, rather than downloading a moving webpage during rendering.

The library adapter is `W3cSpecificationFile`. See [the specification guide](../usage/specification-renderer.md)
for publication steps and [ReSpec compatibility](../other/architecture/respec-compatibility.md)
for the supported migration contract and known limits. W3C branding is source
metadata, not a requirement imposed on unrelated publications.
