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
    ga:url "https://example.org/spec/" ; ga:order 1 .
```

Use ordinary Markdown and the same page frontmatter as a documentation site.
The content before the first level-two heading forms the hero. Level-two headings
start sections; level-three headings within a section start cards. Deeper headings,
code examples, notes, links, tables, and images keep their normal meaning.
No HTML, JSX, or project-specific renderer code is required in the source repo.

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
