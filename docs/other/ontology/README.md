---
title: "Ontology"
layout: default
parent: Other
nav_order: 4
permalink: /ontology/
---

# Ontology

The publication vocabulary separates documentable subjects and shared fragments
from target-specific placements. Its contract is defined in
[the delivery specification](../architecture/deliverables.md).

- [Publication vocabulary](publication.ttl)
- [Publication SHACL constraints](publication.shacl.ttl)
- Executable project fixtures: `tests/fixtures/publication/projects.ttl`
- Rust validation and domain interfaces: `src/publication`

The fixtures cover an ontology family with two HTML targets and one PDF target,
a cyclic use-case tree with bounded traversal, and a fund portfolio with an
exclusion. The same introduction component has different placements in three
profiles. External references are explicit and do not imply scope membership.
Scope resolution is a later deliverable; these are input definitions, not a
claim that those scopes are already executable.

The previous [documentation vocabulary](documentation.ttl) and
[component categories](doc-component-categories.ttl) are prototype artifacts;
they do not define the new publication pipeline.
