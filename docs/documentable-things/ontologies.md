---
title: Ontologies and RDF resources
parent: Documentable Things
nav_order: 2
permalink: /documentable-things/ontologies/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:ontologies
---

# Ontologies and RDF resources

RDF collectors retain assertions and named-graph identity from supported RDF
serializations. The ontology publication pipelines recognise explicit
`owl:Ontology` declarations and document named resources associated with the
selected vocabulary.

The family reference recognises these resource categories:

- Classes declared as `owl:Class` or `rdfs:Class`, and RDFS datatypes.
- RDF properties and OWL object, datatype, and annotation properties.
- `owl:NamedIndividual` instances, with their declared classes and other roles.
- Named SHACL shapes and SKOS resources, with their own reference categories.
- Other explicitly typed resources belonging to a selected vocabulary.

This describes source declarations; it does not infer a resource's type by its
spelling or by executing ontology reasoning. A named individual that also has
another declared role retains those assertions without acquiring duplicate
reference anchors.

## Source descriptions and relationships

Reference fragments retain identifiers, labels, definitions, annotations, and
relationships to other resources. English or untagged source descriptions are
used by the current reference pipeline. The EKGF `markdownDescription`
annotation supplies richer authored explanations alongside the RDF.

Anonymous restrictions and RDF collections are retained as part of the named
resource's description. They are subject to explicit depth and size budgets.
External imports remain source assertions; they are not fetched implicitly.
Missing or competing annotations can be exposed in a
[quality report](../usage/quality-review.md).

Ontology identity and namespace are separate metadata. A single source's
ontology IRI need not be identical to its vocabulary namespace. Publisher,
profile, scope, and section hints determine the publication rather than
changing the ontology's identity.

## Publish a reference

The [ontology family workflow](../usage/ontology-families.md) works with selected
vocabularies and explicit family membership. Choose the
[specification renderer](../usage/specification-renderer.md) for a standards
reference, or [bookshelves](../usage/bookshelves.md) for module-sized books.
