---
title: Queryable documentation graph
parent: Features
nav_order: 3
permalink: /features/knowledge-graph/
grapharch:
  subject: urn:grapharch:docs:topic:features:knowledge-graph
---

# Inspect the knowledge behind the publication

GraphArch collects source statements, authored fragments, generated
explanations, relationships, publication hints, and provenance into an RDF
knowledge graph. A generated page or PDF is a view of that graph, rather than
a separate copy of the documentation's meaning.

## Query content and provenance

Use the publication CLI's SPARQL query operation to inspect the KG. You can ask
which fragments explain a resource, which source supplied an annotation, which
components a target selected, or which quality findings remain in a revision.
The [publication pipeline guide](../usage/publication-pipeline.md) shows the
query, planning, archive, and rendering commands.

Source contributions remain separate. Git inputs retain the selected commit
and file location; local working-tree inputs identify the content actually
read. RDF identities, authored text, and generated explanations stay connected
instead of being flattened into an untraceable HTML export.

## Publish an exact revision

Assembly accepts a complete source revision. Planning selects a publication's
content and order against that revision. Rendering then reads the pinned plan
and KG, so a source edit during rendering cannot silently change part of the
result.

A later build can accept new sources while earlier plans remain readable.
Specifications, companion sites, and PDFs can share an input revision while
retaining their own content selection and arrangement.

## Retain and restore the graph

Export the KG and restore it into an empty store to render without reopening the
original files or contacting the original image hosts. The archive includes
ownership and revision metadata as well as publication content; it is an
administrative archive, not automatically a filtered public data download.

A local persistent store supports repeated builds. A compatible transactional
RDF4J service can also hold the documentation KG. This requires stronger
isolation guarantees than an ordinary SPARQL query/update service; see
[SPARQL services](../usage/sparql-endpoints.md) before choosing that backend.
