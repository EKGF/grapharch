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

The namespace is `https://ekgf.org/ontology/grapharch#`. The preferred prefix is
`ga`. The ontology version is aligned with the GraphArch application release;
its canonical [Turtle source](publication.ttl) declares the current version IRI
and development status.

- [Ontology in Turtle](publication.ttl): classes and properties with English
  definitions, ontology membership, domains, ranges, scope notes, and RDF examples.
- [Companion SHACL constraints](publication.shacl.ttl): structural constraints for
  project definitions and selected derived records.
- [Pipeline contract](../architecture/deliverables.md): stage boundaries and
  required publication behavior.

## The conceptual model

A **Project** configures collectors and documentation generators. Collectors
read source content; documentation generators turn collected RDF into reusable
fragments. The ontology names their shared definitions `ga:Documentor` and
their project-bound uses `ga:DocumentorUse`. Renderers then
present the selected fragments as HTML or PDF.
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

OWL and RDFS define the vocabulary's classes, properties, and logical
relationships. The companion SHACL shapes express constraints on publication
definitions, such as required properties and allowed values.

For example, a configured collector use must supply exactly one
`ga:collectorType` IRI and one `ga:source`. These requirements are expressed in
`ga:CollectorUseShape` in the [SHACL file](publication.shacl.ttl).

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

GraphArch directly reuses `artgov:Authority` from the imported Artifact
Governance Ontology. It is a party role, distinct from the organization or other
party playing that role. `ga:publisher` points to an instance of that class.
GraphArch uses the registered EKGF and W3C authority identities rather than
constructing them from URLs.

The [ontology-family pipeline](../../usage/ontology-families.md) uses the same
registry's family identities, artifact references, explicit memberships, and
aliases. Its first fixture publishes all 24 registered EKGF ontologies together.

## Reused vocabularies in the specification

The GraphArch specification also documents selected terms from the vocabularies
it uses. These terms retain their original IRIs and definitions:

| Vocabulary | Selected terms | Use in GraphArch |
|---|---|---|
| Artifact Governance | `artgov:Authority`, `artgov:authorityAcronym` | Publishing authorities and their abbreviations |
| OMG Commons | `cmns-pts:PartyRole`, `cmns-rlcmp:isPlayedBy` | Distinguishing an authority role from the organisation playing it |
| Dublin Core Terms | `dct:title`, `dct:description`, `dct:source`, `dct:identifier` | Titles, explanations, source attribution, and identifiers |
| PROV-O | `prov:Entity`, `prov:wasDerivedFrom`, `prov:atLocation` | Source provenance in the exported documentation graph |

The [publication definitions](https://github.com/EKGF/grapharch/blob/main/docs/grapharch.ttl) identify the upstream RDF
sources and select these terms with `ga:include`. The generated reference gives
the original definitions and their sources alongside GraphArch's own terms.
Selecting a term for documentation is separate from declaring an ontology import.

## Maintenance

`Cargo.toml` is the release-version source. After changing its package version,
run `python3 packaging/distribute.py sync-version` to update both the ontology's
version metadata and npm package versions. `python3 packaging/distribute.py check`
rejects version drift before distribution. Synchronising these files does not
publish a release or change the namespace.

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
