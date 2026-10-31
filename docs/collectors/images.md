---
title: Image Collector
parent: Collectors
nav_order: 8
permalink: /collectors/images/
grapharch:
  subject: urn:grapharch:docs:collectors:images
---

# Image Collector

The Image Collector retains supported PNG, JPEG, and SVG images, including
their bytes, identity, source revision, and digest. The same image can then
appear in HTML and PDF after the original source is removed.

## What it documents

Images illustrate [Markdown pages and guides](../documentable-things/authored-documentation.md)
and [published specifications](../documentable-things/published-specifications.md).
They can also supply a publication's product logo.

## Include an illustration

```markdown
![Product lifecycle](https://example.org/assets/lifecycle.svg)
```

Supported Markdown collection workflows collect the referenced image separately.
Repository Markdown can resolve a relative path such as
`../assets/lifecycle.svg` within the selected Git revision. Rendering restores
the retained bytes without fetching the image again. Supply meaningful
alternative text. SVG validation rejects active content and external references.

## Configure a product logo

The DPROD showcase's source-owned publication definitions demonstrate an
explicit image job:

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

This uses that showcase's configured publishing workflow; it is not a standalone
`--kind image` CLI command or automatic family-build discovery. Product logos
are separate from authority logos and render at 100 CSS pixels in the
specification header. See the
[DPROD publication definitions](https://github.com/EKGF/dprod/blob/develop/publication/standards.ttl)
for the complete working example.
