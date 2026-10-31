---
title: Features
nav_order: 3
permalink: /features/
grapharch:
  subject: urn:grapharch:docs:topic:features
---

# Publish documentation from the knowledge you maintain

GraphArch turns authored explanations and structured RDF sources into connected
websites and review documents. Maintain the content once, then choose how each
audience should read it: a specification, a companion website, a PDF, or a
library of module books.

## What GraphArch does

| Feature | What it gives your project |
| --- | --- |
| [Collect existing sources](sources/README.md) | Read Markdown, RDF files, repository content, and selected results from SPARQL services. |
| [Reusable content and hints](content-and-hints.md) | Connect explanations to stable subjects and arrange them differently for each publication. |
| [Queryable documentation graph](knowledge-graph.md) | Inspect the content, relationships, provenance, and exact revision behind a publication. |
| [Websites and PDFs](targets/README.md) | Publish final static HTML and typeset documents from the same collected knowledge. |
| [Quality review](quality-review.md) | Show competing annotations and missing descriptions to reviewers without discarding source statements. |
| [Local builds and CI](workflow.md) | Use the same CLI locally and in automation, with explicit failures and ownership of generated files. |
| [Extend GraphArch](extensibility.md) | Add focused adapters and publication styles without making every source or output follow one format. |

## Choose what to document

The [Documentable Things catalogue](../documentable-things/README.md) describes
how GraphArch recognises authored content, ontologies, ontology families,
constraints, taxonomies, and use-case trees. Source location and documentable
subject are separate choices: an ontology can live in a file, a repository, or
a SPARQL service.

## See the result before configuring it

Browse the [showcase gallery](../showcases.md) for generated publications, then
follow [Getting started](../usage/getting-started.md) to install the CLI and
choose a publication guide. These pages describe implemented capabilities;
new source types and richer specialist interfaces require further development.
