---
title: Authored documentation
parent: Documentable Things
nav_order: 1
permalink: /documentable-things/authored-documentation/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:authored
---

# Authored documentation

A Markdown-file collector reads one authored file. Frontmatter can give its
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
[SPARQL services](../usage/sparql-endpoints.md).

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
