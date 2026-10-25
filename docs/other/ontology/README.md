---
title: "Ontology"
layout: default
parent: Other
nav_order: 4
permalink: /ontology/
grapharch:
  subject: urn:grapharch:docs:topic:other:ontology
---

# Grapharch documentation and publication ontology

Read the [GraphArch Ontology Specification](../../spec/develop/index.html),
including the generated class and property reference, examples, and downloadable PDF.

The Grapharch vocabulary describes the documentation pipeline itself. Domain
ontologies such as DPROD retain their own classes and properties; Grapharch links
their resources to reusable fragments, logical components, publication targets,
and recorded build results.

The namespace is `https://ekgf.org/ontology/grapharch#`, conventionally written
`ga:`. The current ontology is a **0.0.1 development draft**, maintained alongside
the implementation. The version IRI is a development identifier; persistent
namespace hosting and immutable release publication remain release work.

- [Ontology in Turtle](publication.ttl): 30 classes and 116 properties, with English
  definitions, ontology membership, domains, ranges, scope notes, and RDF examples.
- [Companion SHACL constraints](publication.shacl.ttl): structural constraints for
  project definitions and selected derived records.
- [Pipeline contract](../architecture/deliverables.md): stage boundaries and
  required publication behavior.

## The conceptual model

A **Project** configures specialized **Collectors** and **Documentors**. Collectors
read native sources; documentors generate fragments from already collected RDF.
Their **Contributions** retain source and processor provenance.

A **Fragment** describes one or more identified subjects. A **Component** groups
fragments into a logical unit. A **Profile** arranges components using ordered
**Placements**. A **Target** selects a scope, profile, renderer, and destination.
The same fragment can therefore appear in a website, a specification, and a PDF
with different arrangements. A specification, companion website, and review PDF
form a [publication family](../../usage/publication-families.md) with distinct targets,
scopes, and profiles.

Assembly accepts a **Revision**. Scope resolution produces a **ScopePlan**, and
publication resolution produces a **Plan** with ordered **Entries** and selected
assets. A **PublicationJob** records rendering against that pinned plan.
**Diagnostics** explain problems and can supply their own documentation fragments.

## Meaning and validation

OWL and RDFS declarations describe meaning and support inference. They do not
reject missing properties or unsafe paths. SHACL and Rust validation enforce
cardinalities, supported values, safety checks, resource budgets, and lifecycle
rules. The SHACL file covers part of the runtime contract; passing those shapes
alone does not guarantee a successful build.

Domains and ranges stay broad where a property is deliberately shared. For
example, `ga:subject` can identify any domain resource. A narrow domain would
infer extra types, rather than act as an input-validation rule. Shared properties
must not inadvertently classify every processor as a collector or every plan as
a target.

Literal revision tokens (`ga:revision`) are distinct from revision IRIs
(`ga:inputRevision`). A captured snapshot pins contribution versions; assembly
must still accept the selected content before it is publishable. These are
Grapharch records, not native Oxigraph revision numbers.

The ontology also documents internal dataset relationships such as `ga:head`,
`ga:pending`, and `ga:data`, because they occur in exported datasets. Their meaning
depends on named-graph context. Consumers should use pinned views rather than
combine every retained version into one graph. Local artifact-directory IRIs
record execution provenance and are not portable public download links.

## Governance alignment and ontology families

`ga:Authority` has the same meaning as `artgov:Authority` from the Artifact
Governance Ontology: it is a party role, distinct from its organization. The
ontology imports that governance vocabulary and declares the class equivalence.
Grapharch uses the registered EKGF and W3C authority identities rather than
constructing them from URLs.

The [ontology-family pipeline](../../usage/ontology-families.md) uses the same
registry's family identities, artifact references, explicit memberships, and
aliases. Its first fixture publishes all 24 registered EKGF ontologies together.

## Maintenance

`tests/publication_ontology.rs` checks ontology metadata, term definitions,
implementation vocabulary coverage, embedded RDF examples, and property kinds in
a collected dataset. Executable publication fixtures live in
`tests/fixtures/publication/projects.ttl`; pipeline interfaces live in
`src/publication`.

The [ontology specification](../../spec/develop/index.html) is generated from the
Turtle vocabulary itself. This companion page introduces the model and links to
the specification and vocabulary downloads.

The earlier [documentation vocabulary](documentation.ttl) and
[component categories](doc-component-categories.ttl) are prototype artifacts;
they are not imported into this ontology and do not define the publication
pipeline contract.
