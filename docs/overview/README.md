---
title: Overview
layout: default
nav_order: 1
grapharch:
  subject: urn:grapharch:docs:topic:overview
permalink: /overview/
---

# From source fragments to publications

Grapharch separates source collection, logical documentation structure, and
publication layout. The shared documentation KG connects all three.

## Collect documentable things

Each collector handles one kind of input. Markdown-file collectors retain prose
and frontmatter; RDF-file collectors retain graph assertions; SPARQL-file
collectors document queries without executing them. Specialized documentors can
generate fragments for ontology classes and other RDF constructs.

A documentation tree also exposes topics, ordered text fragments, and code
examples. A topic has an identity in the KG; its source filename is provenance,
not its meaning.

## Assemble and query the graph

Collection writes through a bounded channel to the graph store. Independent
collectors run concurrently. Embedded Oxigraph writes are coordinated, and a
failed input cannot silently replace the last complete contribution.

Assembly validates a project revision. Scopes select content, and publication
profiles arrange components. A resolved target plan pins the content and ordering
that a renderer will consume.

## Publish for the reader

A documentation website supports exploration across topics. A standards document
needs publication metadata, references, conformance sections, and stable anchors.
A PDF needs pagination, a table of contents, and reviewable document structure.

Those outputs can use the same logical fragments while applying different
arrangements. Grapharch's own [website build](../usage/self-hosting.md) is a
working example of this separation.

## Follow implementation progress

The [architecture](../other/architecture/README.md),
[design goals](../design-goals/README.md), and
[implementation status](../other/status.md) describe current behavior and work
that remains, including external stores and incremental editor previews.
