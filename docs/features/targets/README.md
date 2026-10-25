---
title: Targets
layout: default
parent: Features
nav_order: 3
permalink: /features/targets/
grapharch:
  subject: urn:grapharch:docs:topic:features:targets
---

# Publication targets

A target selects a scope of the documentation KG, arranges logical components
through a publication profile, chooses a renderer, and owns an output directory.
Several targets can reuse the same fragments while presenting them differently.

| Output | Renderer | Artifacts |
| --- | --- | --- |
| [Documentation website](html.md) | `website` | A static HTML page per grouping, shared CSS, and navigation |
| [Reader HTML](html.md) | `html` | A single HTML document and CSS |
| [Formal specification](../../usage/specification-renderer.md) | `specification` | Standards-style HTML/CSS and a paired PDF |
| [Review document](typst.md) | `typst` | Typst source and a PDF |

The built-in HTML renderers write final HTML/CSS directly from Rust. The result
can be served by any static host, including GitHub Pages. There is no additional
site-generator build stage.

See [Self-hosted documentation](../../usage/self-hosting.md) for the website
build that generates this documentation.
