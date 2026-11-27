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
ambiguous, because one is a box and the other an edge or attribute. When two
different class boxes would show the same name, for example `Capability` from
two namespaces of one ontology, each of those boxes, and only those, adds the
UML `(from <vocabulary>)` line under its name, using the vocabulary's prefix.
Colliding names that a vocabulary line cannot separate, and colliding property
names, gain a QName qualifier instead; original IRIs remain in links, HTML
tooltips and the retained graph.

## Turn class diagrams off

A publication profile can turn off the class-diagram fragment renderer:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
<urn:example:profile> a ga:Profile ;
    ga:disableRenderer ga:OwlClassDiagramRenderer .
```

Documentation built with that profile, by `publication build-family` for its
primary target or by `publication build-site` for a target with
`ga:ontologySource`, then generates no class diagrams. Classes keep their
reference text, and the build needs neither PlantUML nor Java. The switch
applies to every output, unlike `ga:classDiagrams false`, which omits already
generated diagrams from a PDF only. `ga:OwlClassDiagramRenderer` is the one
value accepted; any other renderer or a literal fails the build. Authored
[PlantUML diagrams](../collectors/plantuml.md) still require the local
renderer. A build without a profile, such as `grapharch build`, always draws
class diagrams, as do `ga:VocabularyReferenceDocumentor` uses.

## What the arrows mean

Named OWL classes use plain UML class boxes. Stereotypes appear only where
they discriminate: `«external»` for a class that the selected inputs use but
do not declare, such as an imported class, `«datatype»` for an RDF datatype,
`«metaclass»` for `rdfs:Class`, `owl:Class` and `rdf:Property` used as a
type (drawn as `Class` and `Property`), and `«enumeration»` for an
enumerated class. No arrow is labelled with an OWL, RDF or RDFS term:

- Asserted `rdfs:subClassOf` between named class references uses a UML
  generalization arrow: its hollow triangle points to the superclass. No
  inference is performed.
- `owl:disjointWith`, and each pair of an `owl:AllDisjointClasses` statement
  that includes the documented class, is the UML constraint `{disjoint}`.
  When both classes are drawn with a generalization to the same parent, both
  generalizations carry `{disjoint}`, forming a generalization set; otherwise
  a dashed line labelled `{disjoint}` joins the two classes.
- `owl:equivalentClass` between named classes is a dashed line labelled
  `«equivalent»`. An equivalence never looks like a generalization.
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
- An object property declared `owl:inverseOf` another is one association with
  both end names: the name at each end is the property that reaches it. The
  inverse's own association is not drawn a second time.
- A property whose single domain is an explicit union of named classes is
  drawn from each member: one association per member, tagged `{xor}`, or the
  attribute on each member. There is no union box.
- A non-datatype property with declared ranges but no domain in the selected
  inputs uses a dotted `«any»` source box and one labelled association
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
- Anonymous OWL class expressions of the documented class, in
  `rdfs:subClassOf`, `owl:equivalentClass` or `owl:disjointWith`, use UML
  notation instead of an expression box. The notation is scoped to the
  restricted class: it never declares a global range or count, and no
  property declaration is invented or import fetched. The mapping is in
  [Class expression notation](#class-expression-notation) below. The same
  restrictions on neighbouring classes shown in the diagram appear as their
  attributes.
- An anonymous expression that is not well-formed OWL, such as an unknown or
  repeated constructor, a malformed count or a malformed RDF list, keeps its
  authored facts as an explicit box named after its form, such as
  "Anonymous OWL restriction 1". A domain or range that is an anonymous
  expression other than an admitted union is drawn the same way.
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
misleading partial assertion. At most eight anonymous class axioms are read,
each with at most four levels of nesting, 24 resources and eight members per
list; an axiom beyond that budget is omitted whole and the diagram is marked
as limited. Full source assertions remain in the publication. Isolated classes
still receive a class box.

Superclasses are placed above the focal class, subclasses below, and associations
laterally where compatible with that hierarchy. Without a hierarchy, both engine
orientations are compared against a balanced 900-by-600 figure area.
The score balances readable label scaling with a penalty for tall, narrow
figures. Layout does not remove
assertions to fit a phone. On narrow screens, the diagram stays readable inside a
keyboard-focusable horizontal scroll region, with an accessible description.
The description is generated from the same UML projection as the drawing: one
plain sentence per drawn element, such as "Core is a kind of Parent." or
"Core is associated with Target by has, multiplicity 1..*, qualified.", never
the underlying triples. PDF figures retain a physical height limit. PDF class boxes,
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

## Class expression notation

| OWL axiom of class `C` | UML |
| --- | --- |
| `C ⊑ R` for a restriction `R` | the restriction notation below |
| `C ≡ R` | the same notation, adding `{defining}` |
| `C ⊑ A ⊓ R …`, `C ≡ A ⊓ R …` | a generalization to each named part `A` (correct, since `C ⊑ A`) and the notation of each restriction part, with `{defining}` for an equivalence |
| `C ≡ A ⊔ B` | `A` and `B` generalize to `C`, marked `{complete}` |
| `C ⊑ {a, b}`, `C ≡ {a, b}` | `C` is an `«enumeration»` with the literals `a` and `b` |
| `C ⊑ ¬ D` | the constraint note `{not D}` |
| `C ≡ D` for a named class `D` | a dashed `«equivalent»` line |
| any other expression | an OCL-style constraint note, such as `inv: self.p->exists(x \| x.q->forAll(y \| y.oclIsKindOf(D)))` |

A restriction filler may itself be an expression:

- A union of named classes, `p some (A ⊔ B)`, is one association end per
  member, each tagged `{xor}`.
- A chain of existentials, `p some (q some C)`, is the path member
  `p.q : C [1..*]`.
- A filler every individual satisfies, such as `owl:Thing` or a zero
  qualified minimum, leaves an unqualified existential: the member `p [1..*]`.
  If the same class has a positive minimum or exact count on that property,
  that count already guarantees the value, so the member is omitted.
- An intersection, complement or enumeration filler, and any other nesting,
  is an OCL-style constraint note.
- A restriction on an inverse property, `[ owl:inverseOf p ]`, is an
  association from the filler to the restricted class, with the multiplicity
  at the filler's end. A data property has no inverse, and a prohibition on an
  inverse property is a constraint note.

An undeclared property in a restriction, such as one from an import that was
not collected, takes the kind of its filler: a datatype filler makes it an
attribute, any other filler an association to that filler's class, drawn
`«external»` when it is undeclared too.

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
