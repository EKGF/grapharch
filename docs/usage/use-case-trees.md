---
title: EKGF use-case trees
parent: Documentable Things
nav_order: 6
permalink: /usage/use-case-trees/
grapharch:
  subject: urn:grapharch:docs:topic:usage:use-case-trees
---

# EKGF use-case trees

## Example: ownership and reuse

```turtle
@prefix uc: <https://ekgf.org/ontology/use-case#> .
@prefix ex: <https://example.org/use-cases/> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

ex:CustomerInsight a uc:UseCase ;
    rdfs:label "Customer insight"@en ;
    rdfs:comment "Understand a customer's situation."@en .
ex:CustomerOverview a uc:UseCase ;
    rdfs:label "Customer overview"@en ;
    rdfs:comment "Present a consolidated customer overview."@en ;
    uc:isPartOf ex:CustomerInsight .
ex:ReferenceInformation a uc:UseCase ;
    rdfs:label "Reference information"@en ;
    rdfs:comment "Supply reusable reference information."@en ;
    uc:isUsedIn ex:CustomerOverview .
```

**Customer overview** belongs to **Customer insight**. **Reference information**
is reused by the overview without becoming its child. The generated reference
shows these distinct relationships and a neighborhood diagram for each case.

Compare the [complete source](https://github.com/EKGF/grapharch/blob/main/examples/use-cases/use-cases.ttl)
with the [generated use-case guide](https://ekgf.github.io/grapharch/showcases/artifacts/use-cases/website/index.html)
and [PDF](https://ekgf.github.io/grapharch/showcases/artifacts/use-cases/review/document.pdf).
Open **Customer overview** and follow its ownership and reuse links.

## Supporting collectors

The [RDF File Collector](../collectors/rdf-files.md) retains the native use-case
assertions. [Repository scanners](../collectors/repositories.md) and the
[SPARQL Endpoint Collector](../collectors/sparql-endpoints.md) are other ways to
supply RDF. The **use-case documentation generator** then recognises explicitly typed EKGF
Use Case instances and generates their documentation and diagrams. The runnable
example below wires that generator into its publication; collecting an RDF
file alone does not select that publication workflow.

## Run the complete example

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

The RDF-file collector retains native assertions. The reusable use-case documentation generator discovers explicit `ekg-use-case:UseCase` instances from a
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
fragments, as the other executable examples do. The documentation generator is reusable;
this does not add a general `build-use-cases` CLI command or a complete
persona/story/workflow documentation engine.

## Authored use-case pages

A documentation project can own its use cases as Markdown pages instead of
maintaining a second RDF copy. Add a specialized source collector:

```turtle
<urn:catalog> ga:collector <urn:catalog:use-cases> .
<urn:catalog:use-cases> a ga:Collector ;
    ga:kind "authored-use-cases" ;
    ga:source "docs/use-case" .
```

The source path is relative to the project root and must be inside the project's
content directory. Each Markdown file becomes an independently collected use
case; the subtree's `index.md` remains an ordinary overview page. Nested
`index.md` files identify their containing use case, while `identity.md`
identifies `identity` beneath its containing directory.

```markdown
---
title: Customer identity
summary: Verify a customer's **identity** and supporting evidence.
is-part-of:
  - ..
is-used-in:
  - /fraud-detection
---

The existing explanation stays here.
```

Titles and summaries are source-owned and required. Summaries retain Markdown
formatting. Ownership accepts zero or one parent; `[]` explicitly declares an
independent root. An omitted ownership field means the containing use case for
nested cases, or an independent root at the subtree's top level. An omitted
reuse field means no asserted reuse.

`..` selects the containing use case, `/fraud-detection` selects a use case from
the configured subtree root, and other relative paths resolve from the current
use-case identity. Missing targets, self references, duplicate source identities,
multiple ownership parents, and ownership cycles stop publication.

GraphArch retains these facts using the EKGF Use Case Ontology, then adds a
**Use-case relationships** topic to each existing page. It contains a native
SVG diagram, the source summary, and links to ownership and reuse neighbors.
The authored page body and existing topic order remain intact; no duplicate
reference pages or generated source files are created. HTML and PDF rendering
use the pinned documentation graph, even after the source tree is removed.

Collection supports up to 256 use-case pages in a subtree, 1024 filesystem
entries, and 64 explicit reuse references per page. Existing native topology
and diagram budgets also apply. Hidden and underscore-prefixed entries are
excluded; symbolic links and paths escaping the source root are rejected.
