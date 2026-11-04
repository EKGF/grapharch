---
title: Markdown File Collector
parent: Collectors
nav_order: 3
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
images use the [Image Collector](images.md). A local Markdown
fragment's image handling and a pinned Git page's relative-file resolution are
distinct entry points; do not assume every arbitrary path is fetched.

## Reuse canonical content in several publications

Give shared content an explicit fragment IRI. Optional section hints map heading
anchors to stable fragment IRIs; the heading anchor is the lowercase title with
spaces replaced by hyphens, with numeric suffixes for repeated headings.

```markdown
---
title: Product guide
grapharch:
  subject: urn:example:product-guide
  fragment: urn:example:content:product-guide
  fragments:
    product-identity: urn:example:content:product-identity
---
# Product guide

## Product identity

Write this explanation once. Both the website and the overview use this text.

## Implementation detail

Keep more detailed instructions available to the website's readers.
```

The whole-body fragment includes all authored Markdown after the frontmatter.
Each section fragment retains its heading and prose through the next heading,
including lists and code blocks. It records `ga:derivedFrom` the whole fragment;
its topic retains hierarchy and source-contribution metadata. A standalone or
pinned Git collector emits these same identified sections when hints are present.
If a configured Git collector also specifies `ga:fragment`, it must agree with
the whole-body hint.

Select the shared section through the existing scope, profile, and placement
model. The profile chooses content and presentation; it does not rewrite prose.

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .

<urn:example:overview-scope> a ga:Scope ;
    ga:include <urn:example:product-guide> ;
    ga:traverse ga:topic ; ga:maxDepth 1 .
<urn:example:overview-profile> a ga:Profile ; ga:style "overview" ;
    ga:placement <urn:example:overview-placement> .
<urn:example:overview-placement> a ga:Placement ; ga:order 1 ;
    ga:component <urn:example:identity-component> .
<urn:example:identity-component> a ga:Component ;
    dct:title "Product identity" ;
    ga:fragment <urn:example:content:product-identity> .
```

The website can retain the whole guide while the PDF selects only product
identity. Editing the canonical paragraph updates both publications. Renderers
consume the retained fragments in the pinned KG, including after export and
restoration; they do not reread or copy the Markdown source during rendering.
Missing section anchors, invalid IRIs, duplicate section IRIs, and conflicting
whole-fragment hints fail explicitly. Each file supports at most 1,024 section
hints, matching the existing section budget.
