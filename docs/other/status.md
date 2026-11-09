---
title: Status
layout: default
parent: Other
nav_order: 98
permalink: /status
grapharch:
  subject: urn:grapharch:docs:topic:other:status
---

# Status

GraphArch generates this documentation website and the publications in the
[showcase gallery](../showcases.md). It supports static websites, ontology
specifications, and PDF documents from a retained documentation graph.

This summary was reviewed on **9 November 2026** against the current source.
It describes development capabilities; a tagged binary may precede them. Check
[release notes](https://github.com/EKGF/grapharch/releases) when choosing a
binary. The website's build identity and provenance identify its generator.

## Available capabilities

| Area | Implemented behavior |
| --- | --- |
| Starting a project | `grapharch build` discovers supported ontology, Markdown, and MkDocs inputs with defaults. Explicit configuration takes precedence. See [Getting Started](../usage/getting-started.md). |
| Source collection | Dedicated collectors handle RDF files and URLs, Markdown, repository publications, SPARQL files and endpoints, images, code, authored PlantUML, and published specification content. Sources retain their provenance. See [Collectors](../collectors/README.md). |
| Structured documentation | OWL classes and properties, SHACL shapes, SKOS taxonomies, use-case trees, and maturity-model data have specialized documentation paths. Ontology annotations provide descriptions and notes. |
| Class diagrams | PlantUML generates clickable local UML views with human-readable labels, inheritance, attributes, associations, supported OWL restriction shorthand, and SHACL-derived counts and types. Original RDF remains retained. See [OWL class diagrams](../features/owl-class-diagrams.md). |
| Websites and specifications | Static navigation, search, cross-references, tables, code highlighting, figures, front matter, appendices, references, and shared branding are supported. Ontology families have an overview and individual ontology specifications. |
| PDF publication | Embedded Typst produces PDF documents from retained publication content, including vector diagrams. HTML and PDF can share a publication plan. |
| Existing documentation | [MkDocs import](../usage/mkdocs.md) translates supported configuration, navigation, Markdown, and assets into GraphArch publications without running MkDocs. |
| Multiple publications | [Publication families](../usage/publication-families.md), [version listings](../usage/publication-versions.md), and [static hosting packages](../usage/static-hosting.md) support related outputs on a shared static host. |
| Documentation graph | Embedded stores support contribution ownership, immutable snapshots, atomic promotion, queries, and export/import. Retained sources and artifacts allow rendering after original sources are removed. |
| Remote sources and storage | SPARQL endpoints can supply source RDF. A writable remote documentation KG requires RDF4J HTTP transactions with verified serializable isolation; query/update-only endpoints are not sufficient. See [SPARQL endpoints](../usage/sparql-endpoints.md). |
| Quality and diagnostics | Source conflicts retain competing values and provenance. Diagnostics and explicit error-publication modes expose failures without claiming a successful clean publication. See [Quality review](../usage/quality-review.md). |
| Distribution and CI | Release binaries and the GitHub Action support automated publication. GraphArch's own CI generates and deploys this website to GitHub Pages. See [GitHub Actions](../usage/github-actions.md). |

## Examples and verification

The executable gallery includes DPROD, OMG Commons, FIBO Business Entities,
EKGF ontologies, SKOS, use-case trees, an independent MkDocs site, and authored
content examples. Its registry declares the sources and available outputs;
not every example provides every output format.

[Repository CI](https://github.com/EKGF/grapharch/actions/workflows/rust-ci.yml)
is the current verification record. Check a run's commit, jobs, and outcome
rather than treating this page as a permanent test-count or pass certificate.
The workflow uses a shared Linux candidate for tests and documentation,
separate operating-system validation, live showcase checks, and Pages deployment.
A failed or incomplete run is not a verified publication.

Regression coverage includes contribution isolation and failure handling,
immutable revisions, source-deleted export/restore rendering, documentation
links and artifact ownership, RDF interpretation, and HTML/PDF generation.
SPARQL integration exercises family publication and export/restore against an
RDF4J NativeStore repository. Browser checks cover navigation, keyboard access,
and responsive layouts; diagram and PDF changes also receive visual review.
These checks do not establish unlimited scale or complete standards coverage.

## Known limits and ongoing work

- **Reasoning:** the publication pipeline does not yet integrate the native
  inference engine. Diagram shorthand interprets supported declarations and
  constraints; it is not general OWL entailment.
- **OWL and SHACL:** UML is a selective view. Complex expressions and unsupported
  restrictions can remain explicit. Neighborhoods are bounded, and the shared
  endnote explains their scope. SHACL information enriches diagrams without
  claiming that instance data has passed validation.
- **Imports:** GraphArch does not automatically retrieve the transitive closure
  of ontology imports. References outside the selected sources can remain external.
- **MkDocs compatibility:** supported content and configuration are translated;
  Python plugins and arbitrary theme code are not executed. GraphArch supplies
  its own presentation. See the import guide for supported constructs and errors.
- **Specification migration:** working DPROD publications and previews do not
  mean every ReSpec behavior has parity or that an authoritative specification
  has been replaced. See the [ReSpec compatibility map](architecture/respec-compatibility.md).
- **Scale and editing:** bounded queues, chunks, source sizes, and artifact limits
  are implementation safeguards, not evidence of arbitrary corpus scalability.
  Live editor/watch workflows and real-time preview performance acceptance are
  not claimed.
- **Release scope:** development-site features may not be present in the latest
  tagged release. Consult the release and build provenance before reproducing a
  showcase with a different binary.

The [architecture and delivery contract](architecture/deliverables.md) remains
the design reference. This status summary does not certify every delivery
criterion as complete.
