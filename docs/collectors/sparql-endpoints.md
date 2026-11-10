---
title: SPARQL Endpoint
parent: Collectors
nav_order: 5
permalink: /collectors/sparql-endpoints/
grapharch:
  subject: urn:grapharch:docs:collectors:sparql-endpoints
---

# SPARQL Endpoint Collector

One endpoint collector runs one explicitly configured `CONSTRUCT` or `DESCRIBE`
request and retains the returned RDF with a source-result revision. It is a
collection source, independently of where GraphArch stores its documentation KG.
A SPARQL `SELECT` result table is not supported as a generic record-to-document
adapter.

## What it documents

Returned assertions support the same [ontology resources](../documentable-things/ontologies.md),
[SHACL shapes](../documentable-things/shacl-shapes.md),
[SKOS vocabularies](../usage/skos-taxonomies.md), and
[EKGF use cases](../usage/use-case-trees.md) as RDF-file input. The response must
actually contain their defining types and relationships.

## Select a source graph

Save `source.yaml`:

```yaml
endpoint: https://data.example.org/sparql
query: |
  CONSTRUCT { ?s ?p ?o }
  WHERE { GRAPH <https://example.org/vocabulary> { ?s ?p ?o } }
max_source_bytes: 8388608
http:
  timeout_seconds: 30
```

Replace the endpoint and graph with your own source, then run:

```sh
grapharch publication collect --store output/kg \
  --project urn:example:project --collector urn:example:vocabulary-query \
  --kind sparql-endpoint --source source.yaml
```

Use distinct collector identities for different queries. Requests have explicit
size/time limits; response content type identifies the RDF serialization. Failed
requests and malformed responses are errors. Separate queries do not promise one
atomic source-endpoint snapshot.

The adapter is `SparqlEndpointSource`. For authenticated requests, protocol graph
parameters, several concurrent family sources, and the separate writable-KG
contract, see [SPARQL endpoints](../usage/sparql-endpoints.md).
