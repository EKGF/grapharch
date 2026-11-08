---
title: SHACL views of classes
parent: Usage
nav_order: 24
permalink: /usage/shacl-class-views/
---
# SHACL views of classes

When collected RDF declares a named `sh:NodeShape` with an explicit
`sh:targetClass`, GraphArch adds that shape's validation view to the matching
OWL class documentation. No extra publication configuration is needed.

Each shape gets its own labeled compartment diagram and constraint table.
Multiple shapes are shown separately: GraphArch does not choose one, merge
their constraints, or imply that the source has passed validation. OWL axioms
and SHACL validation constraints remain distinct views.

Simple IRI property paths can show their declared `sh:minCount`, `sh:maxCount`,
`sh:datatype`, `sh:class`, and `sh:nodeKind`. Missing counts stay missing;
GraphArch does not manufacture `0..*` multiplicities. Repeated property shapes
on the same path remain separate, and contradictory integer count declarations
are identified without silently choosing a value.

Complex paths, logical combinations and qualified constraints remain in the
linked full shape definition and constraint table. Their property shape is
omitted as a whole from the simple compartment view, with an explicit count.
Other constraints, including SPARQL rules, are referenced rather than converted
into misleading UML field notation. Untyped resources, property shapes used as
node targets, anonymous node shapes and inferred target associations are not
selected by this exact named-shape discovery rule.

Discovery is bounded to eight shapes per class, thirty-two property shapes per
shape and sixty-four direct SHACL facts per shape. Exceeding a limit is an error,
not a partial silent publication.

The collected RDF retains the original constraints. The documentation graph
also retains each view's class and shape identities, selected field facts,
PlantUML source, renderer identity and SVG. HTML and PDF consume that retained
view, so restoring an exported graph requires neither the original files nor
PlantUML. Figure text has a corresponding linked table for keyboard access and
print readers.
