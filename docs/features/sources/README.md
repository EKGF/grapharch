---
title: Collect existing sources
parent: Features
nav_order: 1
permalink: /features/sources/
grapharch:
  subject: urn:grapharch:docs:topic:features:sources
---

# Collect existing sources

Keep documentation close to the project it describes. GraphArch uses focused
collectors for particular inputs, so a publication can combine authored prose
with RDF resources and other supported constructs. Collecting content does not
choose its chapter, page, or publication status; those are publication decisions.

## Available source paths

| Source | What is collected |
| --- | --- |
| [Markdown](files/markdown.md) | Authored explanations, headings, code examples, frontmatter, and supported images or diagrams. |
| [RDF files](files/rdf.md) | Native statements and annotations that describe resources such as ontologies, constraints, and taxonomies. |
| [Git repositories](endpoints/git-repo.md) | Existing local checkouts or a selected remote revision, with file and commit provenance. |
| [SPARQL services](endpoints/sparql.md) | RDF returned by explicitly configured `CONSTRUCT` or `DESCRIBE` queries. |

There is also a specialised collector for supported structures in an
already-published W3C/ReSpec HTML specification. It reads the published document
without running its original JavaScript generator. See
[ReSpec compatibility](../../other/architecture/respec-compatibility.md) for its
scope and the [specification guide](../../usage/specification-renderer.md) for
an example.

## Combine inputs deliberately

Use an ontology-family build for a directory, repository, or set of endpoint
queries. Use a website build for a Markdown documentation tree. The staged CLI
can also collect individual inputs into a shared documentation graph before
planning a publication. The [workflow guide](../workflow.md) links these paths.

Independent collectors can run concurrently. Each input keeps its own identity
and observed revision. An updated source replaces its own successful
contribution; a missing file, invalid RDF, or failed request is an explicit build
failure rather than permission to publish an incomplete replacement.

## Content support is specific

A file extension alone is not a promise that GraphArch understands the file's
meaning. Generic CSV, JSON, XML, YAML, TOML, SQL, Cypher, and GraphQL conversion
adapters are not currently supplied. Configuration files and retained code
examples in those formats are different from collectors that document arbitrary
records or database schemas. See [extending GraphArch](../extensibility.md).
