---
title: HTML and CSS
parent: Targets
nav_order: 1
permalink: /features/targets/html/
grapharch:
  subject: urn:grapharch:docs:topic:features:targets:html
---

# HTML and CSS

Grapharch's built-in Rust renderers generate final HTML and CSS directly from a
pinned documentation graph and publication plan. The generated files are ready
for static hosting, including GitHub Pages.

## Documentation websites

The `website` renderer arranges selected logical topic fragments into pages.
Navigation, breadcrumbs, page contents, and previous/next links are generated
from KG metadata. Local links and heading references must resolve.

Grapharch's own documentation is a working example. See
[Self-hosted documentation](../../usage/self-hosting.md) for the build command,
frontmatter, graph model, and integration tests.

## Single-document HTML

The `html` renderer produces a single document with a table of contents and a
reader-oriented layout. A publication profile controls component placement and
ordering independently of source filenames.

## Formal specifications

The `specification` renderer produces a familiar standards-style document with
authority branding, publication metadata, numbered sections, references, and
appendices. It also produces a paired PDF from the same plan, linked through a
PDF download icon.

See [Specification renderer](../../usage/specification-renderer.md).

## Publish the generated files

Serve the target's output directory with a static HTTP server or deploy it to a
static host. Grapharch stages the complete generation and promotes it atomically;
a failed generation preserves the previous publication.

Markdown is an authored input format. The HTML publication path does not require
Jekyll, MkDocs, Sphinx, a JavaScript framework, or an additional generator.
