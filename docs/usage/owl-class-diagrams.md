---
title: OWL class diagrams
layout: default
nav_order: 9
---

# OWL class diagrams

GraphArch automatically adds a small class-neighborhood diagram to each OWL
class documented from collected RDF. This works in ontology-family publications,
DPROD publications, and an unconfigured `grapharch build` of an ontology file.
There is no separate diagram source to maintain. Collection/documentation uses a
local PlantUML installation on `PATH` and Java. The embedded Smetana layout
engine does not require a Graphviz executable. GraphArch never downloads an
engine or sends ontology content to a diagram service; a missing local engine
is an explicit error. Rendering an already documented graph needs neither tool.

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

The diagram is deliberately local. It shows up to five complete relationship groups within
an 18-fact budget, selected in deterministic IRI order. If both engine orientations
produce a drawing wider than 600 pixels, whole groups are removed from the end
until the figure fits that width. The limit notice identifies these omissions.
Domain/range facts for a
property and facts describing a direct restriction stay together. An oversized
group is omitted whole, with a visible limit notice; it is never cut into a
misleading partial assertion. Full source assertions remain in the
publication; nested expressions beyond the direct neighborhood are not expanded
recursively. Isolated classes still receive a class box.

## Reproducible publication

The documentor reads the pinned collected RDF revision and stores the typed
neighborhood, generated PlantUML source, engine version, and validated SVG in the
generated documentation fragment. HTML and PDF use that same retained vector
drawing; target-specific links are resolved without rerunning layout. Rendering an exported and restored documentation KG does
not require the original ontology files or network access. Changes to incoming
relationships also change the generated fragment content and revision.

Wide neighborhoods are also laid out top-to-bottom by the same engine; GraphArch
retains the narrower of the two standard orientations. Labels wrap without
discarding identifier characters. It does not position edge captions itself.

The build reuses one bounded local renderer process across class diagrams. It
restarts the process after 128 requests and limits each request’s time and output.
The renderer runs with PlantUML’s sandbox security profile.

This automatic feature is independent of [authored PlantUML diagrams](../collectors/plantuml.md),
which remain supported by the dedicated collector and documentor.
