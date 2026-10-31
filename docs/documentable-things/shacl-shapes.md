---
title: SHACL shapes
parent: Documentable Things
nav_order: 4
permalink: /documentable-things/shacl-shapes/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:shacl
---

# SHACL shapes

## Example: require one customer name

```turtle
@prefix ex: <https://example.org/customer#> .
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

ex:CustomerShape a sh:NodeShape ;
    sh:name "Customer shape"@en ;
    sh:description "Describes the required name of a customer."@en ;
    sh:targetClass ex:Customer ;
    sh:property [
        sh:path ex:name ;
        sh:datatype xsd:string ;
        sh:minCount 1 ;
        sh:maxCount 1
    ] .
```

The generated **Customer shape** entry documents its target class and the
nested constraint: one string value for `ex:name`. Readers can inspect the
rule without interpreting a blank-node identifier. This documents the rule;
it does not claim that any customer data has passed validation.

Compare [DPROD's shape sources](https://github.com/EKGF/dprod/tree/develop/ontology)
with the **SHACL shapes** sections in its
[generated specification](https://ekgf.github.io/grapharch/showcases/artifacts/dprod/specification/index.html).

## Supporting collectors

The [RDF File Collector](../collectors/rdf-files.md) retains shape assertions
from a file. [Repository scanners](../collectors/repositories.md) discover RDF
files, and the [SPARQL Endpoint Collector](../collectors/sparql-endpoints.md)
can collect shape assertions returned by an explicit query. The reference
documentation stage then recognises `sh:NodeShape` and `sh:PropertyShape`.

## Recognised constraints

The RDF documentation stages recognise explicitly typed `sh:NodeShape` and
`sh:PropertyShape` resources. Named shapes have reference entries with their
source labels, descriptions, identifiers, and retained constraint statements.
They can be documented alongside the ontology they constrain.

`sh:name` and `sh:description` supply the shape's display name and description.
These describe the shape itself; a property shape's name does not replace the
canonical label of the property identified by `sh:path`. `skos:prefLabel`,
`rdfs:label`, Dublin Core titles/descriptions, and source comments are also
recognised when supplied.

Every retained SHACL predicate can be displayed as an RDF fact; documentation
is not limited to a whitelist of constraint predicates. Core constraints,
SPARQL constraints, and additional SHACL predicates retain their source IRIs
and values. This includes targets, messages and severity, property paths,
logical combinations and lists, ordering/grouping, and rule expressions.
Showing these statements does not claim that GraphArch executes each SHACL
extension. In particular, a target class is presented as `sh:targetClass`, not
as an inferred `rdfs:domain`.

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
