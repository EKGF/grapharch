---
title: Bookshelves and module books
parent: Usage
nav_order: 11
permalink: /usage/bookshelves/
grapharch:
  subject: urn:grapharch:docs:topic:usage:bookshelves
---

# Publish an ontology family as a bookshelf

A large family can be published as a collection of module books. Each book has
its own specification, ontology chapters, and PDF. The bookshelf groups books
by their declared parent modules. Inside a book, the normal specification table of contents shows its ontology
chapters and their sections. It is present in the generated HTML, including
sections in other chapters, before JavaScript runs. The shared specification
controls provide collapse, expand, resize, and search. Search on the bookshelf covers the
family; search inside a book covers that book.

On an existing family target, replace the renderer setting and add optional PDF
controls:

```turtle
<urn:example:specification>
    ga:renderer "bookshelf" ;
    ga:destination "specification" ;
    ga:pdfWorkers 2 ;
    ga:pdfTocDepth 2 .
```

All required specification metadata still applies. Run the normal
`grapharch publication build-family` command. In JSON mode, the result includes
`books_path` and `book_count`; the collection-level `pdf_path` is null. Individual
PDF routes, statuses, checksums and reuse flags are in `books.json` and the
publication manifest.

## Choose book boundaries

Collectors preserve explicitly declared module memberships. The publication
planner records the resulting `ga:BookPlan` resources in the pinned plan:

- Leaf modules become books by default, with their ontologies as chapters.
- Parent modules act as series linking their child books.
- Ontologies without a selected module book remain individual books in the
  **Additional ontologies** catalogue group. They are not discarded or assigned
  to modules by filename or import dependencies.

Hints on module resources override these defaults:

```turtle
<urn:example:domain> ga:bookBoundary true .
```

`true` makes this module and its descendants one larger book. `false` keeps a
module as a series. Nested explicit `true` boundaries are ambiguous and fail
planning; choose one boundary for that subtree. Hints can be supplied in the
ontology RDF or the project's separate RDF definitions. Apply them to the module
resource itself, which may be a named individual, rather than its metadata
ontology declaration.

`ga:pdfTocDepth` controls printed PDF contents independently of HTML
`ga:tocDepth`. Books default to depth two: chapters and sections. This keeps large
term dictionaries out of the printed contents without removing their definitions
from the book. Values from one to eight are accepted.

## Parallel PDF compilation

One coordinator collects and documents the sources once, pins the revision, and
resolves all book boundaries. It generates HTML and prepares each book's Typst
job from that KG. A bounded pool of child **Grapharch CLI processes** compiles
the PDFs; workers do not clone repositories, open the coordinator's KG, or write
to it. The coordinator records the results.

`ga:pdfWorkers` accepts one to eight workers and defaults to two. Larger prepared
jobs start first, so one large book can compile while another worker completes
smaller books. A book has a 30-minute compilation deadline. Cancellation and
fail-fast errors terminate outstanding worker processes. Large individual books
can still require considerable time and memory; splitting a family does not
split the contents of a very large ontology.

Rust library callers must configure the worker binary explicitly when the
current executable is not the Grapharch CLI:

```rust
let publisher = Publisher::new(dataset)
    .with_book_workers(grapharch_binary, 2)?;
```

## Reuse, provenance, and failures

Unchanged PDFs are reused after verifying their checksums and effective input
fingerprints. Those fingerprints cover the prepared Typst content, compiler
implementation and dependencies, embedded fonts, and retained image assets.
Changing one ontology can invalidate only its book's PDF; changing shared
publication metadata or assets can invalidate more books. HTML and publication
metadata are regenerated from the selected revision.

The KG records each book operation as `ga:BookPublication`, connected to its
parent publication job with `dct:isPartOf`, its book plan with
`prov:wasDerivedFrom`, and its input revision with `ga:inputRevision`. Generated
artifact entities carry their locations and PDF checksums; `ga:reused` records
verified reuse. No worker writes directly to these graphs.

A failed PDF blocks the complete publication in fail-fast mode. With explicit
`--keep-going`, successful PDFs and HTML can be published with diagnostics;
failed books show a failure state and have no PDF download link. Diagnostics
also go into the KG against the observed input revision. Previously generated
PDFs with different content are never advertised as current results.

The complete generation is promoted atomically. A failed or superseded run
cannot replace the last successful publication. Rendering can run entirely from
a restored exported KG after the source repository has been removed.

The [FIBO example](https://github.com/EKGF/grapharch/tree/main/examples/fibo)
includes preparation and build commands for this publication model.
