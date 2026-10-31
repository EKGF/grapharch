---
title: Extend GraphArch
parent: Features
nav_order: 7
permalink: /features/extensibility/
grapharch:
  subject: urn:grapharch:docs:topic:features:extensibility
---

# Extend a focused part of the workflow

Projects differ in where their knowledge lives and how they want to publish it.
GraphArch separates collecting inputs, generating explanations, choosing a
publication's content, and rendering the result. A new source type does not
need to dictate the website layout, and a new renderer can reuse already
collected content.

## Add a specialised source adapter

A collector should understand one specific input or kind of documentable
content. It contributes identified RDF statements, fragments, source hints, and
provenance. Independent collectors can run concurrently while the shared writer
respects the store's write constraints and configured resource bounds.

This is how the existing Markdown-file, RDF-file, repository, endpoint, image,
and published-specification adapters remain focused. New record-conversion or
database-schema adapters would be implemented and tested as additional
collectors.

## Reuse the documentation model

Generated explanations retain the identity of the resource they describe.
Publication definitions choose which resources and fragments belong together,
their reading order, and the required metadata. A renderer consumes the resolved
plan and pinned KG rather than reopening the original source systems.

A new presentation can therefore reuse existing collected content, provenance,
references, and quality findings. The documentable resource, the authored
explanation, and the rendered page are distinct parts of the model.

## Current extension boundary

The current CLI distributes the built-in collectors and renderers together.
There is no installed-at-runtime conversion-plugin system, generic database
adapter, or executable MDX component environment. Adding those capabilities
requires implementation work; putting an arbitrary plugin name in frontmatter
does not enable them.

Contributors can start with the [architecture](../other/architecture/README.md),
[delivery contract](../other/architecture/deliverables.md), and
[code contribution guide](../other/contribute/contribute-code.md). Include an
example that also tests collection, provenance, and rendering from an exported
KG after the original sources are removed.
