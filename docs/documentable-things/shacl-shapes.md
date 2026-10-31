---
title: SHACL shapes
parent: Documentable Things
nav_order: 4
permalink: /documentable-things/shacl-shapes/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:shacl
---

# SHACL shapes

The RDF documentation stages recognise explicitly typed `sh:NodeShape` and
`sh:PropertyShape` resources. Named shapes have reference entries with their
source labels, descriptions, identifiers, and retained constraint statements.
They can be documented alongside the ontology they constrain.

A property's path, datatype, class, node kind, cardinality, allowed values, and
other supplied constraint statements remain RDF facts. Nested anonymous shapes,
logical expressions, and RDF lists are displayed within the named resource's
description, subject to the documentation engine's depth and size budgets.
There is no need to invent a named documentation entry for each blank node.

Documenting a shape does not execute SHACL validation against a data graph.
Likewise, retaining a `sh:sparql` constraint documents its statement; it does not
run that statement during rendering. Validation results from an independently
run validator are not implied by a generated shape reference.

## Publish shapes

Include the shape sources and the related vocabulary in a selected
[ontology family](ontology-families.md). Reference section hints can name and
order the Shapes section. The same collected shape statements can appear in a
[specification and PDF](../usage/specification-renderer.md), with links to
other documented resources.

Source-file, repository, and endpoint configuration belong to the
[collection and publishing workflows](../usage/README.md); the shape's meaning
comes from its RDF assertions, independent of that transport.
