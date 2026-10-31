---
title: Markdown pages and guides
parent: Documentable Things
nav_order: 1
permalink: /documentable-things/authored-documentation/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:authored
---

# Markdown pages and guides

Write a getting-started page, a user guide, or an explanation of an ontology
term in Markdown. GraphArch can publish that content as website pages and
reuse it in specifications and PDFs.

## Example: a getting-started page

Save this as `getting-started.md`:

````markdown
---
title: Getting started
permalink: /getting-started/
grapharch:
  subject: urn:example:guide:getting-started
---

# Getting started

Our customer vocabulary gives teams a shared definition of a customer.

## Before you begin

- Read the vocabulary overview.
- Choose the terms your application needs.

> [!TIP]
> Keep explanations beside the vocabulary they describe.
````

The website gets a **Getting started** page with a contents list and a styled
tip. The stable subject lets the same explanation be selected for another
publication. The file alone is content; the project's publication definitions
choose its output and placement.

For a complete working project, compare the
[Markdown source](https://github.com/EKGF/grapharch/blob/main/examples/mixed-sources/introduction.md)
with its [generated guide](https://ekgf.github.io/grapharch/showcases/artifacts/mixed/website/index.html)
and [PDF](https://ekgf.github.io/grapharch/showcases/artifacts/mixed/review/document.pdf).
To build that example from the GraphArch checkout:

```sh
cargo run --example publish_showcase -- --mixed
```

## Supporting collectors

The [Markdown File Collector](../collectors/markdown-files.md) reads a Markdown
file and its frontmatter. Website discovery and the
[repository scanners](../collectors/repositories.md) select Markdown files for
their specialised collectors. Supporting images use the
[asset collectors](../collectors/assets-and-code.md).

Documented queries are a separate kind of source: the
[SPARQL File Collector](../collectors/sparql-files.md) reads a query file and its
documentation comments. It does not execute the query.

## Recognised content

Frontmatter can give a Markdown file's
subject a stable identity and supply page titles, navigation, and publication
hints. Headings and sections become logical topics and ordered fragments;
fenced code blocks also become queryable examples in the KG.

GitHub-flavoured Markdown includes tables, task lists, strikethrough, and labelled
callouts. See [Markdown source support](../features/sources/files/markdown.md)
for syntax and boundaries. MDX execution and arbitrary embedded scripts are not
part of this support.

## Statements, diagrams, and assets

A specialised SPARQL-file collector retains statement text and supported
documentation pragmas as a fragment. It documents the query rather than
executing it. An endpoint collection request is a different workflow; see
[SPARQL endpoints](../usage/sparql-endpoints.md).

Supported fenced Mermaid flowcharts are documented as static SVG assets. PNG,
JPEG, and supported SVG images are also retained in the KG. Renderers use the
retained bytes instead of reopening the source or contacting an image server.
The current Mermaid subset is flowcharts; other diagram types are rejected.

An authored fragment can explain an ontology term, introduce a family, supply
legal or review material, or become a guide page. Its subject identity is
independent of its file name and its eventual place in a website or PDF.

## Publish authored content

Use [self-hosted documentation](../usage/self-hosting.md) for a documentation
website, or the [publication pipeline](../usage/publication-pipeline.md) to
assemble authored and generated fragments into explicit target plans.
