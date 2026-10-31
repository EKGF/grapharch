---
title: Documentable Things
nav_order: 3
permalink: /documentable-things/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things
---

# Documentable Things

GraphArch can document authored explanations and the things described by RDF.
This catalogue lists the kinds that the current collection and documentation
stages recognise. Recognition uses explicit source structure, RDF types,
relationships, and publication hints. A target's scope and profile decide which
of those things appear in a publication.

| Documentable thing | What GraphArch recognises today |
|---|---|
| [Markdown pages and guides](authored-documentation.md) | Markdown pages, topics, sections, fenced code examples, images, and supported Mermaid flowcharts |
| [Ontologies and RDF resources](ontologies.md) | OWL ontologies; OWL/RDFS classes, datatypes, properties, and named individuals; supplied labels, definitions, and relationships |
| [Ontology families and modules](ontology-families.md) | Explicitly registered families and members, plus declared modules and their membership |
| [SHACL shapes](shacl-shapes.md) | Named node and property shapes, with their retained constraints and nested RDF expressions |
| [SKOS taxonomies](../usage/skos-taxonomies.md) | Concept schemes, concepts, collections, and ordered collections, with source hierarchy, labels, notes, and mappings |
| [EKGF use-case trees](../usage/use-case-trees.md) | Actual EKGF Use Case instances, ownership and reuse links, related resources, and generated neighborhood diagrams |
| [Published specifications](published-specifications.md) | Published W3C specification structure, metadata, sections, and examples |
| [SPARQL queries](sparql-queries.md) | Query text and documentation comments, published without executing the query |
| [Legal preambles](legal-preambles.md) | Supported legal-disclosure blocks, with complete wording preserved for HTML and PDF |

## Which collector supports each thing?

A **documentable thing** is what readers learn about. A **collector** is the
source reader that brings its content into the documentation graph. One RDF
file can contain several documentable kinds, and the same kind can arrive from
a file, a Git repository, or a SPARQL endpoint.

| Documentable thing | Supporting collectors |
|---|---|
| Markdown pages and guides | [Markdown File Collector](../collectors/markdown-files.md), with [asset collectors](../collectors/assets-and-code.md) for images |
| Ontologies, SHACL shapes, SKOS taxonomies, and EKGF use cases | [RDF File Collector](../collectors/rdf-files.md) or [SPARQL Endpoint Collector](../collectors/sparql-endpoints.md); [repository scanners](../collectors/repositories.md) discover and dispatch file collectors |
| Ontology families and modules | [RDF File Collector](../collectors/rdf-files.md) or [SPARQL Endpoint Collector](../collectors/sparql-endpoints.md), retaining explicit family registry and module declarations |
| Published W3C specifications | [Published W3C Specification Collector](../collectors/published-specifications.md) |
| SPARQL queries as documentation | [SPARQL File Collector](../collectors/sparql-files.md) |
| Legal preambles | [Legal Preamble Collector](../collectors/legal-preambles.md) |

The [Collectors catalogue](../collectors/README.md) lists each source reader,
its supported inputs, and the documentable things it can supply. The
documentation stage recognises retained RDF types and relationships and builds
the reference entries or diagrams; a generic RDF reader alone does not decide
which publication to generate.

## Try a practical example

Each type guide includes input you can inspect, an explanation of the result,
and a link to generated output. Start with a
[Markdown guide](authored-documentation.md#example-a-getting-started-page), a
[customer vocabulary](ontologies.md#example-a-customer-vocabulary), a
[SHACL rule](shacl-shapes.md#example-require-one-customer-name), or a
[use-case tree](../usage/use-case-trees.md#example-ownership-and-reuse).

The same semantic content can arrive through an RDF file, a repository, or an
explicit SPARQL collection request. Those are ways to obtain sources; they do
not define what the resources mean. See [Usage](../usage/README.md) for transport,
configuration, CI, and publishing workflows.

## Recognition and presentation

A collector retains source content and provenance in the documentation KG.
A documentation generator describes a particular kind of resource from that collected graph.
An assembled revision and its target plans then drive the renderers. See the
[staged architecture](../other/architecture/README.md) for these boundaries.

Support for a resource type does not imply every specialist presentation already
exists. For example, the current SKOS support documents vocabulary resources and
their source relationships. A purpose-built taxonomy browsing or print renderer
is a separate presentation capability, not an existing tree or A–Z index.

GraphArch keeps source statements rather than inventing missing explanations or
fetching ontology imports implicitly. The [quality report](../usage/quality-review.md)
helps reviewers find missing annotations and competing source statements.

## Extend the catalogue

The list can grow. Contributors can add a small, specialised collector for one
kind of source or a documentation generator for one kind of thing in the KG, retaining stable
identities and source provenance. Existing target plans and renderers can then
reuse those fragments, or a specialist renderer can provide a different reading
experience.

The [collector interface](https://github.com/EKGF/grapharch/blob/main/src/publication/collect/mod.rs)
and [documentation-generation interface](https://github.com/EKGF/grapharch/blob/main/src/publication/document/mod.rs)
are the current extension points. See [Contributing code](../other/contribute/contribute-code.md)
and the [delivery contract](../other/architecture/deliverables.md) for contribution,
bounded execution, revision, and test requirements.

