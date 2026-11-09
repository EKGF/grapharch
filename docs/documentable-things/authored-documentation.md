---
title: Markdown pages and guides
parent: Documentable Things
nav_order: 1
permalink: /documentable-things/authored-documentation/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:authored
---

# Markdown pages and guides

Explain how a vocabulary works, introduce a project, or help someone complete a
task. Authored documentation gives readers the context that a generated term
reference cannot supply on its own. GraphArch can publish those explanations as
website pages and select the same content for specifications and PDFs.

## Example: a getting-started page

A guide can lead readers from the vocabulary's purpose to their first decision:

```markdown
# Getting started

Our customer vocabulary gives teams a shared definition of a customer.

## Before you begin

- Read the vocabulary overview.
- Choose the terms your application needs.

## Describe a customer

Use the Customer class for the person or organization your team serves.
Keep the customer identifier stable when their contact details change.
```

The website presents a **Getting started** page with a contents list and links
to related material. A specification can include its introduction, while a PDF
can include the full guide. The publication plan decides which content appears
and where; moving the source file need not change its subject identity.

## See a complete publication

Compare the [authored source](https://github.com/EKGF/grapharch/blob/main/examples/mixed-sources/introduction.md)
with the [generated guide](https://ekgf.github.io/grapharch/showcases/artifacts/mixed/website/index.html)
and its [PDF](https://ekgf.github.io/grapharch/showcases/artifacts/mixed/review/document.pdf).
That example combines an explanation with structured source material, rather
than maintaining separate website and print copies of the prose.

An authored page can also introduce an ontology family, explain a term, or
provide a review narrative. Illustrations and examples can accompany that text;
they remain available when publishing from an exported documentation graph.

## Collect and publish a guide

The [Markdown File Collector](../collectors/markdown-files.md) is the reference
for authoring syntax, frontmatter, images, diagrams, reusable sections, and
validation boundaries. It explains how to give the guide a stable identity and
how page hints control navigation independently of the source filename.

Use the [website build guide](../usage/self-hosting.md) to publish a collection
of guides, or the [publication pipeline](../usage/publication-pipeline.md) to
assemble authored and generated content into selected publication targets.
