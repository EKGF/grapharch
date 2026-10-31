---
title: Ontologies and RDF resources
parent: Documentable Things
nav_order: 2
permalink: /documentable-things/ontologies/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:ontologies
---

# Ontologies and RDF resources

The [RDF File Collector](../collectors/rdf-files.md) reads RDF files and retains
their assertions and named-graph identity. The ontology publication pipelines recognise explicit
`owl:Ontology` declarations and document named resources associated with the
selected vocabulary.

## Example: a customer vocabulary

```turtle
@prefix ex: <https://example.org/customer#> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .

<https://example.org/customer> a owl:Ontology ;
    rdfs:label "Customer vocabulary"@en .

ex:Customer a owl:Class ;
    rdfs:isDefinedBy <https://example.org/customer> ;
    rdfs:label "Customer"@en ;
    skos:definition "A person or organisation receiving our products."@en .
```

When this vocabulary is selected for publication, **Customer** gets a class
reference entry with its definition and IRI. Adding labelled properties and
named individuals supplies entries in their respective sections. This is a
source example; the [family workflow](../usage/ontology-families.md) adds the
namespace, membership, and publication configuration.

For a real vocabulary, compare
[DPROD's sources](https://github.com/EKGF/dprod/tree/develop/ontology)
with its [generated specification](https://ekgf.github.io/grapharch/showcases/artifacts/dprod/specification/index.html).
Look at the **DataProduct** class and its links to related resources.

## Supporting collectors

| Source | Collector or discovery path |
|---|---|
| An RDF file on disk | [RDF File Collector](../collectors/rdf-files.md) |
| RDF files in a local or remote Git repository | [Repository scanners](../collectors/repositories.md), which select the specialised RDF-file collectors |
| RDF returned by a configured query | [SPARQL Endpoint Collector](../collectors/sparql-endpoints.md) |

These collectors retain RDF. The ontology documentation stage recognises the
declared resource types and turns the selected vocabulary into reference
entries. Reading RDF and generating those entries are separate steps.

## Recognised resources

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
