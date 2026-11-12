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


## EKGF website presentation

GraphArch's own documentation selects the EKGF website theme in its source
publication profile:

```turtle
doc:profile a ga:Profile ; ga:style "ekgf" .
```

The theme uses the EKGF graph mark, OMG identity, indigo and orange palette,
and locally bundled Roboto font. Its identity credits and font license ship
with the generated website. It does not fetch fonts or logos while readers
browse the site. Other websites retain their default presentation unless they
select this theme; specification and PDF authority branding remain separate.


## Independent website navigation

For a documentation site, select `ga:layout "documentation"` on its profile.
The layout keeps site identity, section navigation, a page tree, and the local
heading outline distinct; `ga:style "ekgf"` independently selects EKGF branding.

A profile can select one `ga:Navigation` for each `ga:navigationRegion`:
`"primary"`, `"sidebar"`, or `"footer"`. Each tree contains identified
`ga:navigationEntry` occurrences. An occurrence has its own `ga:order`, optional
`ga:parentEntry`, and optional `dct:title` label. It refers to a `ga:page` or a
`ga:url`. A section without a destination must have children and a title.
Parent entries must belong to the same tree.

A `ga:url` is either an absolute `http`, `https` or `mailto` URL, or a URL
relative to the website root, such as `spec/main/` for a publication deployed
beside the website. GraphArch links a relative URL relative to each page, so it
keeps working wherever the hosting profile mounts the site; the definitions
never name the deployment location. A leading slash would bypass that mount
and is rejected, as are backslashes, parent (`..`) segments and any target
that resolves outside the website root. `ga:headerLink` entries follow the
same rules, without `mailto`.

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .

<urn:profile> ga:layout "documentation" ; ga:navigation <urn:sections> .
<urn:sections> a ga:Navigation ; dct:title "Sections" ;
    ga:navigationRegion "primary" ;
    ga:navigationEntry <urn:introduction>, <urn:concepts>, <urn:specification> .
<urn:introduction> a ga:NavigationEntry ; ga:order 1 ;
    dct:title "Introduction" ; ga:page <urn:home> .
<urn:concepts> a ga:NavigationEntry ; ga:order 2 ;
    dct:title "Concepts" ; ga:page <urn:concept-page> .
<urn:specification> a ga:NavigationEntry ; ga:order 3 ;
    dct:title "Specification" ; ga:url "spec/main/" .
```

For existing content, an entry can use `ga:sourcePath "index.md"` instead of
`ga:page`: GraphArch resolves the source-relative path to the collected page
identity before planning. This avoids adding identifiers to every source file.
A missing page or a combination of source path and explicit destination is an
error. The resolved identity is retained in the graph.

Set `grapharch.subject: urn:home` in the home Markdown frontmatter to give its
page that identity. The same page can appear in another tree or twice in the
same tree with different entry identities and labels. This does not duplicate
its content or change the PDF reading order. `ga:headingAnchor` on an entry
selects an existing heading on its destination page. Explicit entries can link
to pages excluded from automatic navigation with `nav_exclude`.

Configured trees replace automatic navigation only in their selected regions.
Existing page-derived navigation still applies when no explicit sidebar tree is
selected. Entry destinations must be selected pages, anchors must exist, sibling
orders must be distinct, and cycles are errors. Trees are limited to 512 entries
and sixteen levels. Navigation is retained in the documentation graph, so restored
publications do not need the original Markdown or configuration files.

This is the shared navigation/layout foundation for MkDocs migration, not yet
an importer for arbitrary MkDocs plugins or complete feature parity. Authored
content remains the source; generated site files do not belong in source repos.


### Reuse directory navigation hints

A `documentation-navigation` collector reads existing `.pages.yaml` files into
these navigation trees. This supports ordered Markdown/directory entries,
label-to-path entries, directory titles, hidden sections, and `...` expansion.
Directory `index.md` pages become their section's destination rather than a
repeated child entry. The original files remain authoritative; no MkDocs runtime
or generated navigation file is needed. For example, with `docs/` as the website
source root:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
<urn:project> ga:collector <urn:directory-navigation> .
<urn:directory-navigation> a ga:CollectorUse ;
    ga:collectorType ga:DocumentationNavigationCollector ; ga:source "." ;
    ga:navigationProfile <urn:profile> .
```

The collector retains the navigation source text and digest in the KG and
produces independent primary/sidebar tree membership. It resolves the page
paths through the same page bindings as explicit RDF navigation. Other MkDocs
plugins, content syntax extensions and executable Python hooks are separate
capabilities; this collector does not execute them.

Documentation website builds infer absent page titles from the first Markdown heading, then the filename. An explicit frontmatter title remains authoritative and invalid frontmatter still fails collection.

Exclude generated files or legacy host instructions from automatic source discovery using project-owned relative paths:

```turtle
<urn:project> ga:excludeSourcePath "generated-diagrams", "CNAME" .
```

Each path must be relative to the documentation root without traversal or symlinks. Excluded generated output need not exist in a clean checkout. Directory exclusions cover their subtrees; this is exact path selection, not a glob language. Definitions are retained before discovery, so these hints are included in provenance and apply before the source budget. Explicit navigation to an excluded page still fails rather than silently dropping the link.

A repository can keep authored pages in `docs/` and RDF dependencies in `metadata/`. Pass the repository as `--source`, set `ga:contentSource "docs"` on the project, and configure collectors relative to the repository root (for example `ga:source "metadata/model.ttl"`). Page routes stay relative to `docs/`. The directory navigation collector should use `ga:source "docs"` too. Source exclusions remain relative to the content directory.

For snippets stored outside the authored page tree, declare explicit additional
source directories on the project, for example `ga:includeRoot "."` alongside
`ga:contentSource "docs"`. Paths are relative to the project root, remain within
it, and are limited to eight directories. Included source text and its provenance
are retained in the graph; rendering does not read those directories.

A documentation site can give an individual authored page the same landing-page
presentation as a project site using `grapharch: {hero: landing}` (or
`hero: introduction`) in its frontmatter. The page keeps the shared community
header and footer; other pages retain their documentation navigation. Existing
`feature_sections`, `resource_sections`, `panel_sections`, and `card_icons`
hints select how ordinary Markdown sections are presented. No second theme or
copy of the page's prose is needed.
