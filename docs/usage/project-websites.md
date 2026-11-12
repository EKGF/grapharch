---
title: Project websites
parent: Usage
nav_order: 15
permalink: /usage/project-websites/
grapharch:
  subject: urn:grapharch:docs:usage:project-websites
---
# Project websites from Markdown

A website profile can select `ga:style "ekgf-project"` to publish a project
landing site with the EKGF identity, without documentation sidebars. The regular
`"ekgf"` style remains appropriate for documentation with a navigation rail.
Both styles generate static HTML, CSS, local fonts, and small navigation scripts;
there is no application framework or Node.js runtime in the published artifact.

```turtle
<urn:example:profile> a ga:Profile ;
    ga:style "ekgf-project" ;
    ga:headerLink <urn:example:specification> .
<urn:example:specification> dct:title "Specification" ;
    ga:url "spec/" ; ga:order 1 .
```

A relative `ga:url`, such as `spec/`, resolves from the website root on every
page and follows the hosting mount. Absolute `http` and `https` URLs link
elsewhere. Host-relative paths, backslashes and parent segments are rejected.

Use ordinary Markdown and the same page frontmatter as a documentation site.
The content before the first level-two heading forms the hero. Level-two headings
start sections; level-three headings within a section start cards. Deeper headings,
code examples, notes, links, tables, and images keep their normal meaning.
No HTML, JSX, or project-specific renderer code is required in the source repo.

A page can select a section as a bordered overview panel using its generated
heading anchor. For example, `grapharch: {subject: urn:example:home,
panel_sections: [what-is-this-project]}` presents `## What is this project?` as a
panel. The same hint can be supplied as
`<urn:example:home> ga:panelSection "what-is-this-project"` in RDF. Only the named
sections receive this treatment; headings, links, and source content keep their
meaning. A missing heading or invalid anchor fails publication.

```markdown
---
title: Example project
permalink: /
nav_order: 1
grapharch:
  subject: urn:example:home
---
# Example project

A short introduction.

[Get started](guide.md)

## Benefits

### One shared source

Publish websites and reference documents from collected content.
```

Build it with `grapharch publication build-site --source content
--definitions publication.ttl --project urn:example:project
--target urn:example:website --output-root output`. The generated directory can
be served on GitHub Pages, Vercel, or any static web host. Internal Markdown links
and retained assets use relative routes, including when the site is served under
a project subpath.

The DPROD pilot keeps its project content in
[publication/project-site](https://github.com/EKGF/dprod/tree/develop/publication/project-site).
Its existing live specification-version service remains separate: replacing a
server-rendered deployment inventory requires an explicit source and freshness
policy, rather than hardcoding a snapshot of active branches in Markdown.

## Preserving earlier page addresses

A page can declare earlier publication-relative addresses in frontmatter:

```yaml
---
title: Getting started
permalink: /guide/
aliases:
  - /getting-started/
  - /old-guide/
---
```

GraphArch retains these hints as `ga:routeAlias` on the collected page. A native
documentor can put the same property on a selected fragment to redirect an old
page address to that fragment's heading. The original content identity and
navigation placement remain independent of these addresses.

Each alias produces a small static HTML redirect with a visible continuation
link, so it works on GitHub Pages without server routing rules. Browser redirects
preserve incoming query parameters and fragment identifiers; otherwise a fragment
alias opens its selected heading. Redirect pages use `noindex` and do not become
navigation entries or sitemap pages. Unsafe paths, duplicate aliases, collisions
with published files, and fragment aliases without a selected heading fail the
publication before its output replaces the previous site.

## Documentation search

Every website layout includes the same search button and keyboard shortcut,
Control-K or Command-K. Search covers titles and retained page text, with links
and matching excerpts. It runs entirely in the browser from a generated local
`search-index.json`; no service or framework is required. Hidden pages, assets,
redirects, and generated publication reports are omitted.

The index is generated from the pinned publication graph and remains usable
when the original Markdown and RDF files are unavailable. The renderer streams
one page at a time and rejects an index exceeding 32 MiB or 10,000 pages. Search
results show at most 20 pages. Failed index loading is reported in the dialog.

## Search metadata

Use a short `description` in each Markdown page's frontmatter. The collector
records it as `dct:description` in the documentation graph; HTML renders it as
the search description and Open Graph description. Page titles supply the Open
Graph title and the project title supplies the site name.

For a public website, give its target an absolute directory IRI:

```turtle
<urn:example:website> ga:thisVersion <https://example.org/project/> .
```

The renderer combines that base with each page's route to emit canonical and
Open Graph URLs, `sitemap.xml`, and `robots.txt`. Canonicals deliberately retain
the configured public domain when the same artifact is deployed as a preview.
Only visible HTML pages enter the sitemap. Hidden pages and generated provenance
or quality reports use `noindex,follow`; assets are omitted. No modification dates
are invented. Invalid canonical bases fail publication.

When a project is mounted below an existing domain (for example `/project/`),
the domain's root `robots.txt` must reference the generated sitemap. Crawlers
read robots rules at the origin root; the project's nested copy does not replace
that host configuration.

## Feature cards

Keep card prose in Markdown and select its presentation in the same frontmatter:

```yaml
grapharch:
  feature_sections: [benefits]
  card_icons:
    one-shared-source: {name: network, tone: primary}
```

`feature_sections` selects level-two headings for centered section titles and
restrained feature cards. `card_icons` maps level-three heading anchors to
optional decorative icons. Available names are `network`, `file-json`,
`git-branch`, and `shield`; tones are `primary` and `accent`. Unknown names,
tones, or heading anchors fail publication. The source prose is unchanged and
can be reused in a PDF without importing website decorations.

Equivalent RDF uses `ga:featureSection "benefits"` and `ga:cardPresentation`
with `ga:headingAnchor`, `ga:iconName`, and `ga:iconTone`. The collected hints
remain in the KG, so rendering does not reopen the Markdown file. Lucide icon
license notices are retained with the generated website's identity credits.

### Resource lists

Select `grapharch.resource_sections: [articles-talks]` to present a level-two section as a single-column list of resources. Its level-three headings remain canonical content headings. Cards use smaller horizontal icons; link-only paragraphs become calls to action with decorative external-link symbols. Select `book-open` in `card_icons` with explicit `primary` or `accent` tones. A section cannot select both feature and resource presentation. These hints are retained in the publication graph and do not duplicate the Markdown prose.

### Page introductions and shared styling

Project pages use a left-aligned introduction by default. A campaign landing page opts into `grapharch.hero: landing`; `introduction` is the other supported value. These are semantic roles, with widths and typography supplied by the shared theme. All ordinary card grids share the same title, muted prose, padding, border and elevation system as feature cards. Resource lists retain their horizontal composition. Plain prose uses a narrower reading measure than card grids.

`card_icons` can also identify a level-two section heading, for example an adoption step or a concept panel. Supported icons include the original network, file-json, git-branch, shield and book-open icons, plus triangle-alert, search, plug, users, megaphone, layers, store, cloud, rocket, folder-open, quote, package, database, file-box, sparkles, vote and circle-check. Decorative icons are bundled with their license and remain explicit source metadata.

## Separate website sections

Set `ga:layout "sectioned"` on a website profile and provide a primary
`ga:Navigation` tree. Its root entries become the secondary navigator tabs.
Each entry selects an existing page with `ga:page`; nested `ga:parentEntry`
entries assign additional page roots to that section. Descendants inherit their
section through the ordinary page hierarchy, so direct links select the same tab
and sidebar as navigation from the home page. Pages must belong to exactly one
section: missing or conflicting ownership fails publication.

The renderer derives each section's sidebar from its selected pages. Do not also
configure an independent sidebar tree. A section with one page has no sidebar;
for example, a gallery landing page can stand on its own while its generated
showcase introductions retain the gallery's active tab. All routes remain the
existing page routes. Section names and destinations are publication data, not
renderer-specific product names. The ordinary `documentation` layout continues
to support independently authored primary, sidebar and footer trees.
