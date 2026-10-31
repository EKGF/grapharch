---
title: Images, code examples and downloads
parent: Collectors
nav_order: 8
permalink: /collectors/assets-and-code/
grapharch:
  subject: urn:grapharch:docs:collectors:assets-and-code
---

# Images, code examples and downloads

Assets and examples use separate small collectors. Retaining a file as an image,
code sample, or download does not parse arbitrary records in it into semantic
documentation.

## What they document

These inputs accompany [pages and guides](../documentable-things/authored-documentation.md)
and [published specifications](../documentable-things/published-specifications.md):
illustrations, accessible product identity, code examples, and downloadable text.

| Adapter | Retained input | Entry point |
|---|---|---|
| Image Collector — HTTP (`HttpImage`) | Validated PNG, JPEG, or supported SVG bytes | Referenced HTTP(S) Markdown images and explicitly constructed library requests |
| Image Collector — pinned Git blob | The same supported image formats from one repository blob | Git Markdown image references, or an explicitly configured image job |
| Code Example Collector (`GitCodeFile`) | UTF-8 example in a supported declared format | Explicit source, fragment, subject, and format configuration |
| Text Download Collector (`TextAssetFile`) | UTF-8 file with a source-relative download route | Website discovery and explicit library configuration |

## Image Collector

```markdown
![Product lifecycle](https://example.org/assets/lifecycle.svg)
```

The image has its own collector, asset identity, validated bytes, digest, and
source revision. Rendering restores those bytes from the pinned KG, without
fetching the URL again. Git Markdown can instead resolve a relative image such
as `../assets/lifecycle.svg` within its pinned repository tree. Image validation
rejects active or external SVG content and enforces byte/dimension limits.

### Configure a product logo

The DPROD source-owned publishing integration supports explicit per-file jobs:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix ex: <urn:example:> .
ex:project ga:collector ex:logo-source .
ex:logo-source a ga:Collector ; ga:kind "image" ;
    ga:source "assets/product.jpg" ; ga:sourcePath "assets/product.jpg" ;
    ga:asset ex:logo .
ex:logo dct:title "Example product" .
ex:specification ga:productLogo ex:logo .
```

This illustrates that integration's configuration contract, not a universal
`--kind image` CLI switch or automatic family-build discovery. Other library
integrations can use `PinnedGitBlob::image_collector`. Product logos remain
separate from authority logos and render at 100 CSS pixels in the specification
header.

## Code Example Collector

A code job in the same configured integration identifies its own source and
subject:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix ex: <urn:example:> .
ex:project ga:collector ex:query-example-source .
ex:query-example-source a ga:Collector ; ga:kind "code" ;
    ga:source "examples/products.rq" ; ga:sourcePath "examples/products.rq" ;
    ga:fragment ex:query-example ; ga:subject ex:product-query ;
    ga:format "application/sparql-query" .
```

The `Code Example Collector` (`GitCodeFile`) requires an explicit subject and
fragment, and one of
`application/ld+json`, `application/json`, `text/turtle`,
`application/sparql-query`, or `text/plain`. It retains the code without executing
it or fetching dependencies; JSON examples are not generic JSON-record adapters.
## Text Download Collector

For a website, keep a text download under the documentation root, for example:

```text
docs/
  index.md
  downloads/
    terms.ttl
    LICENSE.txt
```

The `Text Download Collector` (`TextAssetFile`) retains those `.ttl` and `.txt`
files at their source-relative routes. It does not turn them into additional
prose pages or run the RDF documentation generators over a file merely retained as a download. See the
[website workflow](../usage/self-hosting.md) and [pipeline guide](../usage/publication-pipeline.md).
