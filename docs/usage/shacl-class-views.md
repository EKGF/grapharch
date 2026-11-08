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

Each targeting shape has a labelled compartment in its class. Simple datatype
properties appear as typed members; simple `sh:class` constraints add object
relationships. Declared counts appear beside the member or association. These
are validation constraints from the named shape, not OWL entailments or a
claim that validation has passed. The diagram links to the source shapes and
property definitions.

Multiple targeting shapes keep separate labels and counts. GraphArch does not
select one arbitrarily or merge contradictory scopes. Repeated property shapes
remain distinct. Conflicting or incorrectly typed counts stay in the detailed
constraints rather than becoming misleading diagram multiplicities. If neither
count is declared, no multiplicity is printed. A missing lower or upper count
otherwise means zero or unbounded, respectively.

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
