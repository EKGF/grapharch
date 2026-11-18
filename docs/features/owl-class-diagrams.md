---
title: OWL class diagrams
parent: Features
nav_order: 9
permalink: /features/owl-class-diagrams/
grapharch:
  subject: urn:grapharch:docs:topic:features:owl-class-diagrams
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

Class names, attributes and association labels use authored `rdfs:label` values.
Selection prefers the requested language (English by default), then its regional
variants, then language-neutral text; a stable lexical choice resolves multiple
labels at the same priority. One rule applies to every class box, association
label and attribute, including the documented class itself:

- A resource from the same ontology as the documented class shows its label
  only; the caption and term table already give the documented class's QName.
- A resource from another ontology shows its label followed by its QName, as in
  `Authority (ekg-artgov:Authority)`, so the reader can see where it comes from.
- A resource without a selected label shows its QName alone.

Two resources belong to the same ontology when both declare `rdfs:isDefinedBy`
and share a defining ontology, or, when either declares none, when their IRIs
share a namespace. A class and a property with the same label are not
ambiguous, because one is a box and the other an edge or attribute. Only
different boxes, or different properties, with colliding names gain a further
QName qualifier; original IRIs remain in links, HTML tooltips and the retained
graph.

## What the arrows mean

Named OWL classes use UML class boxes marked `«owl:Class»`. Explicit
OWL/RDFS labels preserve relationships that have no direct UML equivalent:

- Asserted `rdfs:subClassOf` between named class references uses a UML generalization
  arrow: its hollow triangle points to the superclass. Anonymous subclass endpoints,
  `owl:equivalentClass`, and `owl:disjointWith` remain explicitly labeled arrows.
  No inference is performed.
- A property is drawn as an edge or a member, never as a box with
  `rdfs:domain` and `rdfs:range` arrows. An object, annotation or generic RDF
  property with a declared domain and range is one labelled association from
  the domain to the range. With several domains or ranges, it is one
  association for each declared domain–range pair: the declarations apply
  together, so each pair holds for every use of the property, not as
  alternative choices. A domain that is not declared as an OWL class in the
  selected inputs, such as an imported `dcat:Dataset`, is still a valid source.
- A datatype property, or a property without a specific range, is an attribute
  row on each declared domain, typed with its declared ranges joined by `and`;
  an untyped row has no range. A domain not declared as an OWL class becomes a
  class box marked with its resource kind when it owns such a row. When a
  domain is an anonymous OWL expression or a range is one, the property is an
  association to that expression box instead. Associations and attributes
  link to the property documentation. This is presentation shorthand: RDF
  properties remain independent resources, and no requiredness, ownership, or
  multiplicity is inferred.
- A non-datatype property with declared ranges but no domain in the selected
  inputs uses a dotted `rdfs:Resource` source box and one labelled association
  per range. This box is a presentation placeholder, not a new RDF assertion
  or a claim that a specific domain cannot be entailed. Declarations in other
  pinned family sources are checked before using the placeholder; subproperty
  inheritance is not executed as an ad hoc inference rule. `rdfs:Resource`
  itself never counts as a specific domain or range.
- A property stays an explicit box only when it takes part in another
  displayed relationship, such as an explicit OWL expression box, or when nothing can own
  it: a datatype property without a domain, or a property with neither a
  specific domain nor a specific range. The retained documentation graph
  preserves every original assertion regardless of shorthand.
- Label text is always drawn literally. Parentheses, angle brackets, quotes,
  guillemets and other PlantUML syntax characters in labels are passed to the
  layout engine as numeric character references, so a label can never become
  a stereotype, template parameter, link or markup.
- OWL class restrictions use UML notation instead of an expression box. This
  covers every direct `rdfs:subClassOf` restriction of the documented class
  whose node has exactly one named `owl:onProperty` and one recognised form,
  and the same restrictions on neighbouring classes shown in the diagram. The
  notation is scoped to the restricted class: it never declares a global
  range or count, and no property declaration is invented or import fetched.
  The mapping is in [Restriction notation](#restriction-notation) below.
- Other direct restrictions retain their `owl:onProperty`, value constraints,
  and cardinalities as explicit relationships: boolean class expressions
  (`owl:intersectionOf`, `owl:unionOf`, `owl:complementOf`), enumerations,
  nested restrictions, inverse properties, property chains, complex fillers,
  malformed counts and restrictions with additional predicates. Each explicit
  box is named after its form and numbered within the neighbourhood, such as
  "Anonymous OWL intersection 1" or "Anonymous OWL restriction 2". Union and
  intersection RDF lists are expanded into labeled member relationships.
- An outer existential whose filler is a simple zero qualified minimum still
  requires a value. GraphArch inspects one additional restriction level within
  the same bounded relationship group, and shows a linked `[1..*]` member. If
  the same class has a positive minimum or exact count on that property, that
  count already guarantees existence, so the duplicate member is omitted.
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
keyboard-focusable horizontal scroll region, with an accessible description. PDF figures retain a physical height limit. PDF class boxes,
member names, and relationship labels are individually clickable. Subjects included
in the PDF link to their internal document anchors; other HTTP(S) resources link
to their original IRIs. In a bookshelf, a subject documented in another book links
to its HTML chapter beside the book PDF, using a relative link. Member hit regions
take precedence over the surrounding class box. The retained drawing and its scale are unchanged; PDF links use native
Typst annotations over the corresponding regions, not rasterized images or a
single link over the whole diagram.

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

## Restriction notation

In the table, `p` is the restricted property, `C` a named filler class, `D` a
named datatype, and `m` the multiplicity. Datatype properties, and data
fillers, use attributes of the restricted class; object properties use
associations from the restricted class to the filler class, with `m` at the
target end. A neighbouring class draws its own restrictions as attributes
only, typed by the filler, so the neighbourhood does not grow new boxes.

| OWL restriction | Datatype values | Individuals |
| --- | --- | --- |
| `owl:minCardinality`, `owl:cardinality`, `owl:maxCardinality` | `p : range [m]` with the declared property range | association to the single declared class range, otherwise `p [m]` |
| `owl:minQualifiedCardinality`, `owl:qualifiedCardinality`, `owl:maxQualifiedCardinality` with `owl:onDataRange` or `owl:onClass` | `p : D [m] {qualified}` | association `m` to `C`, `{qualified}` |
| `owl:someValuesFrom` | `p : D [1..*] {qualified}` | association `1..*` to `C`, `{qualified}` |
| `owl:allValuesFrom` | `p : D {all values}` | association `*` to `C`, `{all values}` |
| `owl:hasValue` | `p {= value}` | association to the individual, `{= value}` |
| maximum or exact count of zero | class constraint `{no p}` | class constraint `{no p}`, or `{no p in C}` when qualified |
| minimum of zero | omitted: it imposes nothing | omitted |

Multiplicities follow one UML convention everywhere, including SHACL counts:
equal bounds show one number (`1`, never `1..1`), an unconstrained range
shows `*` (never `0..*`, and never omitted, because an attribute without a
multiplicity would read as exactly one), and other ranges show
`lower..upper` (`0..1`, `2..*`).

A count of zero is not drawn as a `[0..0]` or `[0]` multiplicity. A member
with an upper bound of zero would show a property the class cannot have, so
the diagram states the prohibition instead, as a UML constraint in the
class box, on a constraint row after its attributes: `{no literal value}`.
A qualified zero excludes only values in the filler, so it reads
`{no start in Date}`: other values of the property remain possible. A SHACL
`sh:maxCount 0` uses the same constraint.

When the restricted class is not a declared domain of the property but an
asserted superclass is, the property is inherited and the restriction
narrows it on the subclass. The member or association then adds
`{redefines}`, meaning it redefines the inherited property of the same name.
For example, `ekg-raw:NullValue` is a subclass of `ekg-raw:Value`, the domain
of `ekg-raw:literalValue`, and has
`rdfs:subClassOf [ a owl:Restriction ; owl:onProperty ekg-raw:literalValue ;
owl:maxCardinality 0 ]`; its box shows the constraint `{no Literal value}`
with no expression box, literal box or property box.

Data counts concern distinct data values, not the number of lexical spellings
of a literal. The original predicates, values and source links remain
retained in the published graph whichever notation is drawn.
