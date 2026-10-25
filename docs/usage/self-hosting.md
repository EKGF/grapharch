---
title: Self-hosted documentation
parent: Usage
nav_order: 4
grapharch:
  subject: urn:grapharch:docs:topic:self-hosting
permalink: /usage/self-hosting/
---

# Grapharch documents itself

This site is a publication of Grapharch's own documentation knowledge graph.
It exercises the production collectors, graph writer, assembler, planner, and
website renderer. There is no separate documentation-site generator.

## Build the site

From the repository root:

```sh
cargo run -- publication build-site \
  --source docs \
  --definitions docs/grapharch.ttl \
  --project urn:grapharch:docs \
  --target urn:grapharch:docs:website \
  --output-root output/docs
```

The build creates a static site in `output/docs/site` and exports its complete
KG to `output/docs/documentation.nq`. Serve the site with any static HTTP server:

```sh
python3 -m http.server 8770 --directory output/docs/site
```

All local links and heading references must resolve. Invalid navigation,
colliding routes, and unsupported constructs are hard errors. The previous
complete site remains available if a later generation fails.

## Model topics, not filenames

Markdown frontmatter supplies a stable logical identity and publication hints:

```yaml
---
title: Self-hosted documentation
parent: Usage
nav_order: 4
permalink: /usage/self-hosting/
grapharch:
  subject: urn:grapharch:docs:topic:self-hosting
---
```

`grapharch.subject` identifies the document in the KG. Keep it unchanged when
renaming or moving the file. `permalink` can preserve the website route across a
move. Titles and parent relationships describe navigation; source paths record
where the content came from.

Within a document, headings identify `ga:Topic` resources. Each topic links to
ordered Markdown fragments; fenced code blocks become queryable `ga:Example`
resources with a language and source provenance. The collector retains the
original document as well as these logical constructs.

Website assembly binds selected topic fragments into publication components.
The renderer reassembles the plan's fragments into pages, resolves links against
the selected graph resources, and builds navigation and page contents from KG
metadata. It never opens a Markdown source file.

## Ontology specification

The project also declares a specification target with
`ga:ontologySource "other/ontology/publication.ttl"`. During collection, the build
reads that Turtle source as RDF, generates reference fragments for its named OWL
classes and properties, and publishes the standards-style reference at
[spec/develop/](../spec/develop/index.html). The target metadata declares its
publisher, draft status, version links, and publication date.

The generated reference retains definitions, scope notes, RDF examples, domains,
ranges, and supported OWL union expressions. Required annotations and unsupported
anonymous expressions are hard errors. The current reference documentor supports
named union members, not arbitrary nested OWL expressions.

Specification HTML, CSS, PDF, Typst, and the preceding dataset export are then
retained as bounded binary assets in the companion website KG. The website
renderer can reproduce the published files after the sources have been deleted.
The spec records its pinned input revision before packaging its generated
artifacts; the companion site's later revision additionally includes those
artifacts. Both plans and their provenance remain in the same dataset.

## Showcase gallery

The [gallery](../showcases.md) contains the six executable showcase fixtures.
Its registry is `examples/showcases.ttl`: each `ga:Showcase` has a title,
description, source, pipeline kind, ordering, and declared HTML/PDF outputs.
The project enables the registry through `ga:showcasesSource "../examples"` in
`docs/grapharch.ttl`.

The CLI builds two independent fixtures concurrently, then specialized
published-artifact collectors retain their HTML, CSS, Typst, PDFs, and dataset
exports in bounded KG chunks. The website plan selects those resources explicitly as `ga:asset` outputs. The
renderer restores the artifacts and checks their byte counts and SHA-256 digests. Gallery cards and sandboxed live
previews are generated from the showcase records in the pinned KG.

The RDF 1.2 fixture declares one expected source diagnostic. The gallery displays
that diagnostic state; unexpected diagnostics fail the build. The gallery's
integration test deletes its copied documentation and example sources, exports
and restores the KG, and checks the resulting HTML previews and PDF downloads.

## Project repository

The header's repository link comes from `ga:repository` on the project resource.
For a GitHub URL, the renderer displays the GitHub icon with an accessible label.
The template contains no project-specific repository URL.

## Inspect the documentation graph

Import the exported dataset into an empty store:

```sh
cargo run -- publication import \
  --store output/docs-store \
  --input output/docs/documentation.nq
```

Save a query such as this to a `.rq` file, then pass it to
`publication query --store output/docs-store --query your-query.rq`:

```sparql
PREFIX ga: <https://ekgf.org/ontology/grapharch#>
SELECT DISTINCT ?topic ?example ?language WHERE {
  GRAPH ?graph {
    ?topic a ga:Topic ; ga:example ?example .
    ?example ga:language ?language .
  }
}
```

The publication manifest records its input revision, plan, target, and owned
artifacts. The integration test exports and restores the KG before rendering,
checks the full repository documentation build, and tests invalid links and
artifact ownership.

## Current boundaries

Discovery supports Markdown pages and UTF-8 Turtle/text downloads. Hidden and
underscore-prefixed directories are excluded. The current limits are 256 source
files, 256 page/download resources, 16 MiB of selected content, and 512 output
artifacts. Showcase collection permits up to 16 declared fixtures and 128
published artifacts, with a 32 MiB limit per artifact and a 128 MiB total budget.
Binary content is streamed through 48 KiB chunks. These are enforced budgets,
not performance measurements.

The website renderer is static HTML and CSS, with accessible navigation and a
responsive reading layout. Search, incremental watch builds, MDX execution, and
automatic PDF books for this website remain separate work.
