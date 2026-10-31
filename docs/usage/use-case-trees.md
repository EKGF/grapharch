---
title: EKGF use-case trees
parent: Documentable Things
nav_order: 6
permalink: /usage/use-case-trees/
grapharch:
  subject: urn:grapharch:docs:topic:usage:use-case-trees
---

# EKGF use-case trees

The [showcase gallery](../showcases.md) includes fictional RDF instances of
`https://ekgf.org/ontology/use-case#UseCase`. It documents actual use cases,
rather than the classes and properties defining the Use Case Ontology.

```sh
cargo run --example publish_showcase -- --use-cases
cargo test --test taxonomy_use_case_showcases
```

The output root is `output/pdf/grapharch-use-cases`: `website/index.html`,
`review/document.pdf`, and `documentation.nq`. The fixture lives in
[examples/use-cases](https://github.com/EKGF/grapharch/tree/main/examples/use-cases).
These source files are original example data, not a migration of EKG Catalog.
The new showcase runner requires a source build containing these changes;
it is newer than the 0.1.0 binary release.

## Semantic relationships

The RDF-file collector retains native assertions. The reusable Rust
`UseCaseDocumentor` discovers explicit `ekg-use-case:UseCase` instances from a
pinned collected revision and emits fragments and SVG assets into the KG.
Renderers read the resolved plan, generated fragments, and retained assets.
They never reopen the source RDF or invoke Python to draw diagrams.

- `isPartOf` is ownership, with zero or one parent per documented use case.
  Multiple independent roots are supported; no synthetic root is added.
- `isUsedIn` is reuse and may link a reusable use case to multiple consumers.
  It is not converted into ownership.
- Stereotypes, personas, outcomes, and concepts retain their source identities,
  labels, and supplied descriptions in reference tables.

English labels and source descriptions are required. The EKGF
`markdownDescription` annotation is supported. Missing or ambiguous text,
dangling ownership/reuse endpoints, multiple ownership parents, and ownership
cycles are hard errors. There is no ontology reasoning or implicit import fetch.
The initial bounded implementation supports 4096 cases, 65536 ownership/reuse
edges, a 16 MiB topology index, ownership depth 32, and up to 128 distinct nodes
in one diagram neighborhood. Descriptions are read one use case at a time.

## Neighborhood diagrams

Each use case has a vector diagram showing ancestors, ownership siblings,
children and grandchildren, plus reuse context where applicable. Children are
sorted by source label. Only the active use case is boxed; ownership lines are
solid and reuse lines dashed. Diagram labels use embedded font metrics.

The diagram is a retained static image; accompanying relationship links provide
navigation between use cases. SVGs contain no active scripts or external image
references. HTML, Typst/PDF, and exported-KG restoration use the same retained
bytes. Retired use cases lose their active generated contributions while
historical snapshots remain available.

The layout is independently implemented in Rust, using the old
[EKG Catalog diagram rules](https://github.com/EKGF/ekg-catalog/blob/main/DIAGRAMS.md)
as a presentation reference. No catalog content or Python code is copied.

The example-specific runner wires a reader target and a PDF target to generated
fragments, as the other executable examples do. The documentor is reusable;
this does not add a general `build-use-cases` CLI command or a complete
persona/story/workflow documentation engine.
