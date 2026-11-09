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
| [Markdown pages and guides](authored-documentation.md) | Guides, explanations, reusable sections, and illustrated narratives |
| [Ontologies and RDF resources](ontologies.md) | OWL ontologies; OWL/RDFS classes, datatypes, properties, and named individuals; supplied labels, definitions, and relationships |
| [Ontology families and modules](ontology-families.md) | Explicitly registered families and members, plus declared modules and their membership |
| [SHACL shapes](shacl-shapes.md) | Named node and property shapes, with their retained constraints and nested RDF expressions |
| [SKOS taxonomies](../usage/skos-taxonomies.md) | Concept schemes, concepts, collections, and ordered collections, with source hierarchy, labels, notes, and mappings |
| [EKGF use-case trees](../usage/use-case-trees.md) | Actual EKGF Use Case instances, ownership and reuse links, related resources, and generated neighborhood diagrams |
| [EKGF maturity models](../usage/maturity-models.md) | Models, pillars, capability areas, capabilities, and levels, with their declared hierarchy and authored companion explanations |
| [Published specifications](published-specifications.md) | Published W3C specification structure, metadata, sections, and examples |
| [SPARQL queries](sparql-queries.md) | Query text and documentation comments, published without executing the query |
| [Legal preambles](legal-preambles.md) | Supported legal-disclosure blocks, with complete wording preserved for HTML and PDF |

## Which collector supports each thing?

A **documentable thing** is what readers learn about. A **collector** is the
source reader that brings its content into the documentation graph. One RDF
file can contain several documentable kinds, and the same kind can arrive from
a file, an RDF URL, a Git repository, or a SPARQL endpoint.

| Documentable thing | Supporting collectors |
|---|---|
| Markdown pages and guides | [Markdown File Collector](../collectors/markdown-files.md), with the [Image Collector](../collectors/images.md) for images |
| Ontologies, SHACL shapes, SKOS taxonomies, EKGF use cases, and maturity models | [RDF File Collector](../collectors/rdf-files.md), [RDF URL Collector](../collectors/rdf-urls.md), or [SPARQL Endpoint Collector](../collectors/sparql-endpoints.md); [repository scanners](../collectors/repositories.md) discover and dispatch file collectors |
| Ontology families and modules | [RDF File Collector](../collectors/rdf-files.md), [RDF URL Collector](../collectors/rdf-urls.md), or [SPARQL Endpoint Collector](../collectors/sparql-endpoints.md), retaining explicit family registry and module declarations |
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

The same semantic content can arrive through an RDF file or URL, a repository, or an
explicit SPARQL collection request. Those are ways to obtain sources; they do
not define what the resources mean. See [Usage](../usage/README.md) for transport,
configuration, CI, and publishing workflows.

## Choose the reading experience

A publication's scope selects the subjects and content to include; its profile
and renderer determine how readers encounter them. The same authored explanation
can introduce a website and appear in a printed specification. An ontology can
have its authoritative reference alongside a separate guide for newcomers.
See [publication targets](../features/targets/README.md) for the available outputs.

Recognition of a resource does not imply every specialist reading experience is
implemented. SKOS resources, for example, have a generated vocabulary reference;
a dedicated interactive taxonomy tree or A–Z browser is a separate capability.
The [SKOS guide](../usage/skos-taxonomies.md) describes the current presentation.

GraphArch retains source statements rather than inventing missing explanations
or fetching ontology imports implicitly. The
[quality report](../usage/quality-review.md) helps reviewers find missing
annotations and competing source statements.

If a kind of subject is missing, see [Contributing code](../other/contribute/contribute-code.md)
and the [delivery contract](../other/architecture/deliverables.md) for adding
specialized collection or documentation support with retained provenance.
