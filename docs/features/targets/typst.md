---
title: PDF via Typst
layout: default
parent: Targets
nav_order: 1
grapharch:
  subject: urn:grapharch:docs:topic:features:targets:typst
permalink: /features/targets/typst/
---

# Target &mdash; PDF via Typst

GraphArch generates a PDF and its editable [Typst](https://typst.app/) source from the same pinned documentation graph as the website. No separate typesetting installation is required.

## Presentation

A PDF paired with a specification uses a standards document layout and the publisher's accent colour. A standalone PDF uses the `ga:style` on its publication profile: `"specification"` selects the standards layout, while `"reader"` selects the companion site's slate and teal palette. The default for a standalone PDF is `"reader"`. Unsupported styles are errors.

Both presentations include a title page, linked contents, running section headings, page numbers, readable code blocks, repeated table headers, and themed GitHub Markdown alerts. Supplied subtitles, publication status, dates, authors, references, appendices, and diagnostic sections remain part of the publication. GraphArch does not invent missing descriptions or publication metadata.

The PDF renderer reads the resolved plan and its pinned graph revision. Regenerating the website and PDF together keeps their content tied to the same source revision.

## Links

- [Typst](https://typst.app/)
- [Typst documentation](https://typst.app/docs/)
