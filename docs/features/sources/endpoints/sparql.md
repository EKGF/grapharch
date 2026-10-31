---
title: SPARQL endpoints
parent: Collect existing sources
nav_order: 4
permalink: /features/sources/endpoints/sparql/
grapharch:
  subject: urn:grapharch:docs:topic:features:sources:endpoints:sparql
---

# Collect selected RDF from a SPARQL endpoint

GraphArch can collect source RDF through explicitly configured `CONSTRUCT` or
`DESCRIBE` queries. Choose the query and dataset that provide the ontology,
shapes, taxonomy, use cases, or other RDF content you intend to document.
GraphArch does not crawl an endpoint or infer which graphs your organisation
wants to publish.

Each configured query owns a separate source contribution. Several queries can
run concurrently and feed the same publication. The result, response format,
source revision, and applicable limits are checked before replacing the last
successful contribution. Authentication can use environment-supplied credentials.

## Separate the source from the documentation store

An endpoint that supplies RDF is not automatically suitable for storing the
GraphArch KG. Source collection works with ordinary SPARQL endpoints returning
supported RDF. A writable documentation store currently requires a compatible
RDF4J transaction protocol with verified serializable isolation.

No source query executes embedded instructions from returned RDF, and ontology
imports are not fetched automatically. Independent requests identify observed
result revisions; they do not claim a shared transactional snapshot of the
source endpoint.

See [SPARQL endpoints](../../../usage/sparql-endpoints.md) for source configuration,
authentication, limits, and backend requirements.
