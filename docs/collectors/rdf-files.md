---
title: RDF File Collector
parent: Collectors
nav_order: 1
permalink: /collectors/rdf-files/
grapharch:
  subject: urn:grapharch:docs:collectors:rdf-files
---

# RDF File Collector

The RDF File Collector is one capability with separate instances for individual
source files. It retains assertions without needing a different collector type
for every ontology or RDF resource kind. Explicit file collection supports
Turtle, RDF/XML (`.rdf`), N-Triples, N-Quads, TriG, N3, and JSON-LD (`.jsonld`).
Directory and repository discovery currently select the first six formats;
JSON-LD needs explicit file collection or an explicitly configured source. Blank-node identities
are normalised for source isolation, and source graph information is retained.
Malformed input fails collection; imports are not fetched implicitly.

## What it documents

Collected RDF feeds documentation generation for [ontologies and RDF resources](../documentable-things/ontologies.md),
[ontology families and modules](../documentable-things/ontology-families.md),
[SHACL shapes](../documentable-things/shacl-shapes.md),
[SKOS taxonomies](../usage/skos-taxonomies.md), and
[EKGF use-case trees](../usage/use-case-trees.md). Explicit types and relationships
matter: a Turtle extension alone does not establish which of those things exists.
Publication hints in RDF can also define projects, scopes, targets, and authority.

## Collect one file

Save this as `terms.ttl`:

```turtle
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
<https://example.org/terms#> a owl:Ontology ;
    rdfs:label "Example vocabulary"@en .
<https://example.org/terms#Product> a owl:Class ;
    rdfs:label "Product"@en ;
    rdfs:comment "An item offered to a customer."@en .
```

```sh
grapharch publication collect --store output/kg \
  --project urn:example:project --collector urn:example:terms \
  --kind rdf --source terms.ttl
```

This collects assertions; it does not yet publish a website. To discover and
publish several RDF files together, use the [family workflow](../usage/ontology-families.md).
Publication definitions select what gets documented and rendered.

The local implementation is `RdfFile`. Pinned repository blobs use the separate
Git RDF-file adapter; [repository discovery](repositories.md) dispatches these
jobs with commit and blob provenance.
