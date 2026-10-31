---
title: Markdown File Collector
parent: Collectors
nav_order: 2
permalink: /collectors/markdown-files/
grapharch:
  subject: urn:grapharch:docs:collectors:markdown-files
---

# Markdown File Collector

A Markdown-file collector retains authored prose and its documentation hints.
It supports GitHub Flavoured Markdown, including tables, task lists, and alerts,
alongside fenced code examples and supported Mermaid flowcharts. It does not
execute MDX or arbitrary embedded JavaScript.

## What it documents

The result is [Markdown pages and guides](../documentable-things/authored-documentation.md):
authored fragments about a subject. In a website build, page frontmatter also
supplies navigation and routes. It does not turn a prose description of an
ontology into RDF declarations for its classes.

## Write a page

```markdown
---
title: Product guide
permalink: /guides/products/
grapharch:
  subject: urn:example:product-guide
---
# Product guide

Explain how publishers describe their products.

- [x] Publish an identifier
- [ ] Review the description

> [!NOTE]
> Keep identifiers stable between publications.
```

Save it under a documentation root that also contains publication definitions.
The [website workflow](../usage/self-hosting.md) uses the page-aware
collector to build that tree. To collect one fragment without website navigation:

```sh
grapharch publication collect --store output/kg \
  --project urn:example:project --collector urn:example:product-guide \
  --kind markdown --source docs/product-guide.md
```

`MarkdownFile::in_site` adds page hints for a website; `GitMarkdownFile` reads a
pinned repository blob and resolves repository-relative references. Retained
images use [separate image collectors](assets-and-code.md). A local Markdown
fragment's image handling and a pinned Git page's relative-file resolution are
distinct entry points; do not assume every arbitrary path is fetched.
