---
title: RDF files
parent: Collect existing sources
nav_order: 2
permalink: /features/sources/files/rdf/
grapharch:
  subject: urn:grapharch:docs:topic:features:sources:files:rdf
---

# Collect native RDF statements

RDF collection retains resource identities, annotations, relationships, and named
graph context. Documentation generators can then recognise
[documentable subjects](../../../documentable-things/README.md) and build
readable explanations from the supplied statements.

## File formats

Directory and repository discovery currently selects:

| Serialization | Extension |
| --- | --- |
| Turtle | `.ttl` |
| RDF/XML | `.rdf` |
| N-Triples | `.nt` |
| N-Quads | `.nq` |
| TriG | `.trig` |
| Notation3 | `.n3` |

The explicit RDF-file collector also accepts JSON-LD through `.jsonld` input.
That does not make arbitrary JSON a documentable-record format, and automatic
family/repository discovery currently uses the six extensions above. XML
catalogue files are not treated as RDF source files.

## Describe what the source actually says

GraphArch preserves supplied labels, definitions, rich descriptions, and
relationships. Source-aware reference generation exposes missing annotations
and competing values as [quality findings](../../quality-review.md). It does not
invent definitions or implicitly download external ontology imports.

Named classes, properties, individuals, shapes, schemes, and concepts retain
their RDF identities. Supported anonymous expressions and RDF collections are
rendered from retained statements, with explicit limits and cycle checks.
Unsupported expressions or malformed input are errors.

See [ontology-family publication](../../../usage/ontology-families.md) for source
selection and [SPARQL collection](../endpoints/sparql.md) for RDF supplied by a
service.
