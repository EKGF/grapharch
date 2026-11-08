---
title: SHACL constraints in class diagrams
parent: Usage
nav_order: 24
permalink: /usage/shacl-class-views/
---
# SHACL constraints in class diagrams

Named `sh:NodeShape` resources with an explicit `sh:targetClass` enrich the
existing OWL class neighborhood. GraphArch does not generate a second shape
box. No additional publication configuration is needed.

Simple datatype properties appear as ordinary UML members, with types and
multiplicities where the declaring shapes agree. Simple `sh:class` constraints
add object relationships; association multiplicities appear at the value end.
Shape names and implementation labels do not appear inside the diagram or its
caption. Source links and exact constraint scopes remain in the detailed view.
These are declared validation constraints, not OWL entailments or a claim that
validation has passed.

Identical declarations are drawn once. If declaring shapes disagree about a
type or count, that qualifier is omitted from the drawing and all declarations
remain separately available in the details. GraphArch never chooses one shape
arbitrarily or silently intersects contradictory counts. Complex or incorrectly
typed counts remain in the detailed constraints. If neither count is declared,
no multiplicity is printed. A missing lower or upper count otherwise means
zero or unbounded, respectively.

Complex paths, logical combinations, qualified constraints and nested node
constraints remain in the linked full definition and detailed constraints.
They are not converted into simple global property multiplicities. Other
constraints, including SPARQL rules, are retained as authored. Anonymous shapes,
untyped target declarations and inferred target associations are not selected
by this exact named-shape discovery rule.

The detail view provides a compact property summary and expandable original
constraint facts. The shared diagram endnote explains notation and bounded
neighborhood selection; those explanations are not repeated beside every
figure. Superclasses sit above their subclasses and associations beside them.

Discovery is bounded to eight shapes per class, thirty-two property shapes per
shape and sixty-four direct facts per shape. One combined drawing admits at
most thirty-two shape scopes and 128 simple fields. Exceeding a bound is an
error, never a silent partial publication.

The collected RDF remains unchanged. The retained diagram includes the original
OWL assertions, each shape scope and property constraint, generated PlantUML,
renderer identity and SVG. HTML and PDF render this same pinned content after
source deletion or graph export/restore, without rerunning PlantUML. The
accessible relationship table preserves OWL facts separately from shape-scoped
constraints; PDF also retains links to the full shape definitions.
