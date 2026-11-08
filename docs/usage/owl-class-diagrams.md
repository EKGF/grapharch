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
resources link to their original IRIs. Accessible SVG descriptions and PDF image
alternatives preserve the meaning without duplicate visible relationship tables.
A shared endnote explains the notation and bounded scope.

## What the arrows mean

Named OWL classes use UML class boxes marked `«owl:Class»`. Explicit
OWL/RDFS labels preserve relationships that have no direct UML equivalent:

- Asserted `rdfs:subClassOf` between named class references uses a UML generalization
  arrow: its hollow triangle points to the superclass. Anonymous subclass endpoints,
  `owl:equivalentClass`, and `owl:disjointWith` remain explicitly labeled arrows.
  No inference is performed.
- A property with one named OWL class as its declared domain can use UML
  shorthand. A datatype property becomes an attribute row, with its declared
  range when present. A property with one declared resource range becomes a
  labelled association; a property without a range remains an untyped member. Both link to the property documentation. This is
  presentation shorthand: RDF properties remain independent resources, and no
  requiredness, ownership, or multiplicity is inferred.
- Multiple domains or ranges, anonymous expressions, missing domains, and
  properties referenced by other displayed relationships stay explicit. Their
  domain/range assertions apply together, not as alternative choices. The
  retained documentation graph preserves every original assertion regardless
  of shorthand.
- A simple anonymous subclass restriction with one explicitly declared object property in `owl:onProperty` and
  one named `owl:someValuesFrom` becomes a class-scoped association, labelled
  with the property and `{at least one Filler}`. This existential qualification
  does not mean every property value belongs to that filler, and does not create
  a global range or an unqualified `1..*` multiplicity. Universal restrictions,
  unknown property types, inverse paths, complex fillers and additional qualifiers stay explicit.
- Other direct restrictions retain their `owl:onProperty`, value constraints,
  and cardinalities as explicit relationships. Union and intersection RDF lists
  are expanded into labeled member relationships. Anonymous expressions have
  distinct identities and numbered captions within the class neighborhood.
- Simple authored SHACL property constraints enrich the same UML attributes and
  association multiplicities. They remain validation constraints rather than OWL
  entailments. Conflicting qualifiers are omitted from the drawing, not silently
  merged. Dedicated shape documentation and the retained graph preserve their
  sources and complete constraints. No validation success, ownership,
  aggregation or composition is claimed.

The diagram is deliberately local. It shows up to five complete relationship groups within
an 18-fact budget, selected in deterministic IRI order. Domain/range facts for a
property and facts describing a direct restriction stay together. An oversized
group is omitted whole, with the scope explained in the shared endnote; it is never cut into a
misleading partial assertion. Full source assertions remain in the
publication; nested expressions beyond the direct neighborhood are not expanded
recursively. Isolated classes still receive a class box.

Superclasses are placed above the focal class, subclasses below, and associations
laterally where compatible with that hierarchy. Without a hierarchy, both engine
orientations are compared against a balanced 900-by-600 figure area.
The score balances readable label scaling with a penalty for tall, narrow
figures. Layout does not remove
assertions to fit a phone. On narrow screens, the diagram stays readable inside a
keyboard-focusable horizontal scroll region, with an accessible description. PDF figures retain a physical height limit.

## Reproducible publication

The documentor reads the pinned collected RDF revision and stores the typed
neighborhood, generated PlantUML source, engine version, and validated SVG in the
generated documentation fragment. HTML and PDF use that same retained vector
drawing; target-specific links are resolved without rerunning layout. Rendering an exported and restored documentation KG does
not require the original ontology files or network access. Changes to incoming
relationships also change the generated fragment content and revision.

Wide neighborhoods are also laid out top-to-bottom by the same engine; GraphArch
retains the balanced orientation selected by the same fit score. Labels wrap without
discarding identifier characters. It does not position edge captions itself.

The build reuses one bounded local renderer process across class diagrams. It
restarts the process after 128 requests and limits each request’s time and output.
The renderer runs with PlantUML’s sandbox security profile.

This automatic feature is independent of [authored PlantUML diagrams](../collectors/plantuml.md),
which remain supported by the dedicated collector and documentor.
