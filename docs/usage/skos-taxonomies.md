---
title: SKOS taxonomies
parent: Usage
nav_order: 12
permalink: /usage/skos-taxonomies/
grapharch:
  subject: urn:grapharch:docs:topic:usage:skos-taxonomies
---

# Document a SKOS taxonomy

RDF-file, local Git, remote Git, and SPARQL collectors retain SKOS assertions in
the documentation KG with their normal source provenance. A taxonomy does not
need an invented OWL ontology declaration. Register its `skos:ConceptScheme` as
a vocabulary in a publication family and use the normal
[`publication build-family` workflow](ontology-families.md).

A standalone scheme supplies its title and namespace metadata, for example:

```turtle
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix sh: <http://www.w3.org/ns/shacl#> .

<https://example.org/business> a skos:ConceptScheme ;
    skos:prefLabel "Business taxonomy"@en ;
    skos:definition "The business vocabulary maintained by this project."@en ;
    sh:declare [ sh:prefix "business" ;
                 sh:namespace "https://example.org/business/" ] .

<https://example.org/business/product> a skos:Concept ;
    skos:prefLabel "Product"@en ;
    skos:altLabel "Offering"@en ;
    skos:definition "An item offered to customers."@en ;
    skos:inScheme <https://example.org/business> .
```

Namespace metadata can also live in the publication definitions. Publication
settings, authority, license, scope, and family registration work as they do for
ontology publications. The existing command and JSON counters retain their
ontology-oriented names when a selected vocabulary is a concept scheme.

## Reference content

SKOS resources have **Concept Schemes**, **Concepts**, and **Collections**
sections. Ordered collections retain their RDF member-list order. The standard
`ga:referenceSection` hints can rename or reorder these sections using the kinds
`schemes`, `concepts`, and `collections`.

English `skos:prefLabel` takes precedence over `rdfs:label`. Conflicting preferred
labels fail documentation. Definitions, alternative and hidden labels,
notations, notes, examples, hierarchy links, scheme membership, collections,
and mappings remain source assertions in the KG. Reference entries show retained
facts and link to other documented resources. Markdown descriptions use the
same annotation support as ontology terms.

For a standalone scheme, term discovery reads the pinned collected-project
snapshot, so concepts can be supplied in separate files. Explicit `skos:inScheme`,
`skos:topConceptOf`, and `skos:hasTopConcept` links select typed resources even
outside the declared namespace. Typed resources in the declared namespace are
also documented. No membership, inverse links, hierarchy closure, or external
imports are inferred.

SKOS schemes embedded in an OWL ontology are documented within that ontology's
reference. A standalone source file currently supports one declared scheme;
multiple standalone schemes in one file, overlapping selected vocabulary
ownership, and ambiguous namespace declarations fail explicitly. Separate
scheme declarations into distinct files and give each selected vocabulary
unambiguous namespace metadata.

## Publication formats

Taxonomies use the specification, paginated specification, and bookshelf
renderers. A standalone scheme can form a book with its own PDF. Publication
works from an exported and restored KG after deleting the original source
files. This is vocabulary-reference documentation; it does not add a dedicated
interactive taxonomy-tree renderer.
