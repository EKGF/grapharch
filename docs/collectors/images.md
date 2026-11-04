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

## Use source-local images

A local website source tree can keep illustrations beside its canonical Markdown:

```markdown
![Product lifecycle](assets/lifecycle.svg)
```

The website collector discovers SVG, PNG, and JPEG files beneath the documentation
root and retains them as bounded binary assets. Relative Markdown image paths
resolve from the Markdown file's directory; their canonical paths must stay
inside that root. Missing files, paths escaping the root, query strings, and
fragment identifiers fail explicitly. The same source file can supply website
and PDF profiles, including selected section fragments, without fetching an
absolute production URL. Rendering reads the validated bytes from the pinned KG.

A standalone local `MarkdownFile` uses its parent directory as the image root.
Embedders collecting standalone inputs can retain a corresponding image with
`LocalImageFile::new(project, root, path).await?`; the full website pipeline
schedules discovered images automatically. HTTPS requests keep their independent
collector and asset-factory behavior. Every image uses the existing 2 MiB,
dimension, and SVG-content limits.

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
