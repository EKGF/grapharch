---
title: Static HTML websites
parent: Websites and PDFs
nav_order: 1
permalink: /features/targets/html/
grapharch:
  subject: urn:grapharch:docs:topic:features:targets:html
---

# Publish final HTML and CSS

GraphArch's built-in renderers produce final static files from a pinned
publication plan and documentation graph. Deploy the output directory to
GitHub Pages or another static host. There is no additional Jekyll, MkDocs,
Sphinx, or JavaScript-framework build step.

## Companion documentation

The companion website arranges logical topics into pages, with navigation,
breadcrumbs, page contents, previous/next links, and source-repository links.
Metadata determines the hierarchy and URLs, so authors can reorganise files
without making filenames the site's information architecture.

The [website guide](../../usage/self-hosting.md) explains the build command and
frontmatter. GraphArch's own documentation exercises this publication path.

## Standards references

The specification style supplies numbered sections, appendices, publisher
branding, publication metadata, and a linked PDF of the same specification.
Its contents sidebar supports resizing, collapsing branches, and search/filter
controls. A paginated specification divides a large reference into smaller pages
with shared navigation and search indexes.

For families with declared modules, the bookshelf style provides an entry point
for separate module books. It avoids forcing every ontology and term into one
giant page. See [specifications](../../usage/specification-renderer.md) and
[bookshelves](../../usage/bookshelves.md).

Current taxonomy support is an RDF resource reference with SKOS relationships.
A specialist taxonomy browser with hierarchy and A–Z views is a separate future
presentation, not an existing feature of these layouts.

## Reliable generation

GraphArch stages output before promotion and preserves the previous complete
publication if generation fails. A target owns its output directory and
manifest; unowned files are not silently overwritten. The generation credit
identifies and links the GraphArch version used.
