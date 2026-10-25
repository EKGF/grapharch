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

Grapharch is implementing the [D0–D6 delivery contract](architecture/deliverables.md).
The new publication pipeline is not yet a usable DPROD replacement.

| Deliverable | Current state |
|---|---|
| D0: architecture | Overview and design goals reconciled with the five-stage shared-dataset design; capability inventory below. |
| D1: ontology | Vocabulary, SHACL definitions, three generic fixtures, target domain interface, and initial Rust validation tests implemented. Full constraint parity and fixture acceptance remain open. |
| D2: dataset and execution | Embedded memory/disk contributions, atomic heads, immutable snapshots, export/import, and a bounded serial write coordinator implemented and tested. Project-isolated assembly validates logical definitions and atomically promotes unchanged, complete inputs; historical validated revisions remain available. External backend, complete stage orchestration, import/query memory verification at scale, abandoned-attempt cleanup, and performance budgets remain open. |
| D3: ingestion | Separate Markdown-file, SPARQL-file, and RDF-file collectors implemented and tested. Initial specialized OWL/SHACL assertion-fragment generators exist; the OWL-class path has tests. A specialized published-W3C-HTML collector now exposes sections, terms, examples, bibliography, native anchors, and publication metadata from a pinned N-Triples fixture. ReSpec authoring-config migration, project configuration, complete annotation/shape interpretation, remain open. A native DPROD fixture generates 53 term fragments from ontology and SHACL annotations. |
| D4: planning | Scope membership, composition, exclusions, and bounded traversal implemented with generic fixture tests. Target plans now record independent placements and fragment order, check internal/external references, and reject overlapping artifact directories. One/all target CLI planning and immutable plan receipts are tested. Richer profiles, explicit nested-target composition, and inclusion explanations remain open. Artifact ownership is checked during publication. |
| D5: publication | Pinned-plan static HTML and embedded Typst/PDF publishing implemented, with specification/reader styles, stable cross references, tables, appendices, glossary, and references. A dedicated publisher-neutral specification renderer adds KG-driven publication metadata, required abstract/status front matter, nested numbered ToC, and permalinks. Reusable authorities supply a validated embedded PNG, name, accent, and publication banner; W3C and EKGF output are tested. Bounded workers, supersession, guarded artifact ownership, restart recovery, and job records are tested. Assets, complete profiles, change annotations, and incremental editor previews remain open. |
| D6: release | Not implemented; native DPROD reference coverage is tested; full specification migration and performance acceptance remain open. |

## Verified foundation

`src/publication` contains the new implementation. Its tests cover definition
validation, project/collector ownership, restart persistence, private staging,
failed and superseded attempts, immutable snapshots, unchanged recollection,
export/import, malformed RDF, normalized authored hints, and channel failure, byte backpressure, source failures before parsing,
default/named graph separation, and cross-project identity isolation.
Anonymous RDF nodes are deterministically skolemized within a source revision.
SPARQL files are documented without execution.

Authored files currently have a 256 KiB limit and RDF files an 8 MiB limit.
Writer chunks are capped at 256 quads and 1 MiB of accounted data. These explicit
initial limits bound per-input work; they are not corpus-size limits or measured
performance claims. Collector scheduling and queue capacity are independently
bounded.

## Current verification

On 2026-10-25, `cargo test --all-targets` passed all 102 tests and
`cargo clippy --all-targets -- -D warnings` passed. Cargo separately
reports the existing unused `aws-sdk-s3` and `printpdf` manifest dependencies.
The new integration suites cover the graph definitions, contribution lifecycle,
collectors, writer channel, snapshot queries, generic scopes, initial OWL-class
documentation, and independent CLI process restarts. The executable showcases share pipeline
code with integration tests: one renders three targets from an exported/restored
KG; another collects Markdown, SPARQL, and OWL concurrently and generates class
documentation. Both render after source removal. See the
[showcase catalog](../usage/examples.md).

The three-page showcase PDF was rendered and visually checked, including repeated
table headers, heading numbering, code, links, glossary, references, and appendix.
Specification and reader HTML were checked in a real browser at desktop/mobile widths.
The EKGF-branded specification and real W3C N-Triples reproduction were checked at
1440px and 390px, with
all local anchors resolving, authority images loaded, no duplicate IDs, and no
horizontal page overflow or browser errors. The real fixture additionally checks
that code blocks, defined terms, table-cell text, reference labels, and explicit
editor order survive the KG round trip. Exact W3C visual parity remains open; see
the [ReSpec compatibility map](architecture/respec-compatibility.md).
No external SPARQL contract test has run. The local
Docker daemon is unavailable; no external backend is implemented yet. There are
no real-time preview performance acceptance measurements. Native DPROD coverage
now includes 53 annotated terms and its dataset-schema worked example; complete
DPROD specification coverage and D6 acceptance are not claimed.

## Prototype boundaries

The original `source`, `loader`, `documentor`, `model`, and `generator` modules
remain available while replacement behavior is tested. Their source-to-book
mapping, separate stores, and unfinished output flags do not implement the new
publication contract. Existing working-tree changes have been preserved.

The `publication` CLI exposes independent validate, collect, snapshot, query,
resolve-scope, assemble, plan, render, export, and import operations. Process-restart
tests cover collection, assembly, planning, and rendering through the CLI. See [usage](../usage/publication-pipeline.md). Complete build orchestration
and watch/editor integration are not implemented. No website deployment or
replacement of DPROD's publication pipeline has been performed.

[![Rust CI](https://github.com/EKGF/GraphArch/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/EKGF/GraphArch/actions/workflows/rust-ci.yml)

[![CLA assistant](https://cla-assistant.io/readme/badge/EKGF/GraphArch)](https://cla-assistant.io/EKGF/GraphArch)


## Diagnostic publications

The primary W3C reference is now the pinned RDF 1.2 N-Triples Working Draft
(23 October 2026). Dedicated specification
publications pair HTML and PDF from one plan, with a PDF download icon. Native
specification content is converted into Typst without executing source scripts.

Collection and rendering errors have KG diagnostic resources and Markdown
fragments. Explicit keep-going collector batches finish independent inputs;
failed sources still block assembly. The specification renderer can publish a
reference-error addendum in HTML/PDF, marks the job published-with-errors, and
the CLI exits nonzero. This does not yet provide complete keep-going orchestration
for every planning/documenting operation.


## Self-hosted website

Grapharch now generates this documentation website through `publication build-site`.
The Markdown collector identifies stable document resources, heading topics,
ordered fragments, and code examples with reusable fragments. Frontmatter supplies
logical IDs, navigation relationships, and routes independently of source filenames.
A specialized UTF-8 asset collector retains vocabulary and license downloads.

The website renderer selects logical topic fragments from the pinned plan and
produces static HTML/CSS with topic navigation, breadcrumbs, page contents, and
previous/next links. Source-relative links and heading anchors must resolve.
A failed build preserves the previous complete output. Website artifact ownership
is bounded and checked recursively before replacement.

The self-documentation integration test collects the complete documentation tree,
removes its copied sources, exports/restores the KG, and renders successfully.
Additional regressions cover stable identities across file moves, broken links,
invalid content, duplicate routes, and unowned nested artifacts. GitHub Pages CI
now builds with Grapharch; Jekyll is no longer the primary site build.


GitHub-flavoured Markdown parsing is shared across collection and publication.
Tables, task lists, strikethrough, and GitHub callouts are supported; HTML callouts
have visible type labels and task-list checkboxes replace ordinary bullets.
Regressions cover all five callout types, nested/mixed task lists, literal markers
inside code, and shared HTML/Typst publication. GSQL and intermediate-generator
targets for Jekyll, MkDocs, and Sphinx are outside the product scope.


## Executable gallery and artifact retention

The showcase gallery reads its five entries from an RDF registry and displays
sandboxed live HTML previews, PDFs, and exported KGs. Example pipelines are shared
library code used by the CLI, executable examples, and tests. Declared fixtures
build with bounded concurrency; an expected-error fixture must match its declared
diagnostic exactly, and unexpected failures stop the build.

Published-artifact collectors retain binary outputs as ordered, bounded chunks in
the documentation KG. Plans select those resources explicitly through `ga:asset`;
they are separate from documentation fragments and component placements. Rendering verifies content size and SHA-256 digests and
publishes those artifacts under the same website ownership manifest. Gallery
rendering succeeds after deleting the copied sources and restoring the dataset.
The project repository URL also comes from the KG; GitHub links display its icon.

Dataset export now buffers serialization through 64 KiB writes. A regression
checks the file-write budget for a large literal, and another checks propagation
of final-buffer write failures. Export still streams the store and atomically
replaces the output file.
