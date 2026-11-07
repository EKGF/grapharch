---
title: OWL class diagrams
layout: default
nav_order: 9
---

# OWL class diagrams

GraphArch automatically adds a small class-neighborhood diagram to each OWL
class documented from collected RDF. This works in ontology-family publications,
DPROD publications, and an unconfigured `grapharch build` of an ontology file.
There is no separate diagram source to maintain and no PlantUML server or Java
installation to configure.

The highlighted box is the documented class. Neighboring classes and properties
link to their documentation when selected in the publication; otherwise HTTP(S)
resources link to their original IRIs. HTML includes an expandable relationship
list as a text alternative. PDF includes vector figures and text links below
nonempty diagrams.

## What the arrows mean

These are UML-style boxes with explicit OWL/RDFS relationship labels, not a
translation of OWL into closed-world UML fields:

- `rdfs:subClassOf`, `owl:equivalentClass`, and `owl:disjointWith` retain their
  source meaning and direction. No inference is performed.
- Properties are separate resources with domain and range arrows. Multiple
  domains or ranges apply together, not as alternative choices. A property
  appearing here is not necessarily required on instances of the class.
- Direct anonymous restrictions retain their `owl:onProperty`, value constraints,
  and cardinalities as explicit relationships. Union and intersection RDF lists
  are expanded into labeled member relationships. Anonymous expressions have
  distinct identities and numbered captions within the class neighborhood.
- SHACL shapes are not presented as OWL restrictions. This view does not assert
  validation requirements, ownership, aggregation, or composition.

The diagram is deliberately local. It shows up to 18 relationships, ordered
by source/predicate/object, in panels of up to six relationships. A visible notice
identifies truncated neighborhoods. Full source assertions remain in the
publication; nested expressions beyond the direct neighborhood are not expanded
recursively. Isolated classes still receive a class box.

## Reproducible publication

The documentor reads the pinned collected RDF revision and stores the typed
neighborhood in the generated documentation fragment. HTML and PDF render that
same retained model. Rendering an exported and restored documentation KG does
not require the original ontology files or network access. Changes to incoming
relationships also change the generated fragment content and revision.

This automatic feature is independent of authored diagrams. Existing authored
image assets remain supported; it does not claim execution support for arbitrary
PlantUML source files.
