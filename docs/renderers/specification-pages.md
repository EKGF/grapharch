---
title: Paginated specification
parent: Renderers
nav_order: 3
permalink: /renderers/specification-pages/
grapharch:
  subject: urn:grapharch:docs:renderers:specification-pages
---

# Paginated specification renderer

A large reference divided into linked HTML pages.

## Output

An HTML library landing page and linked sections with shared navigation and search. This mode does not produce a combined PDF; select a separate PDF target when needed.

## Page routes

The home page is `index.html`. Every other page is the `index.html` of its own
directory, named by a page slug:

| Page | Slug |
| --- | --- |
| A placement with `ga:pageSlug` | That value |
| An ontology-family ontology with `ga:pageSlug` | That value |
| Any other ontology-family ontology | The namespace prefix the family binds with `sh:declare` |
| An ontology-family module with `ga:pageSlug` | That value |
| Any other page | Its title, lowercased, with every run of characters other than ASCII letters and digits replaced by one `-` |

For example, the EKGF use-case ontology, whose family prefix is `ekg-use-case`,
is published at `ekg-use-case/index.html`.

A declared slug or prefix is used unchanged. It must be 1 to 128 lowercase ASCII
letters and digits, in runs separated by single `-` or `_` characters, and must
not be `assets`. Publication fails, naming the page and its subject, when a
declared value is invalid, a title yields no slug, or two pages claim the same
slug. Declare a slug on the ontology or module itself, in its source or in the
project definitions:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
<https://example.org/ontology/use-case#> ga:pageSlug "use-cases" .
```

Anchors within a page are derived from the publication plan, not from the route,
so a slug change moves the page without changing its fragment identifiers.

## Select this renderer

Set the renderer on a publication target in your existing project configuration:

```turtle
<urn:example:target> ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationPagesRenderer> .
```

This is a target setting, not a complete project definition. The target also needs
its scope, profile, destination, and any metadata required by its publication
style. A target can reuse content selected from the same graph as other targets.

See [Specification setup](../usage/specification-renderer.md) for supported presentation and configuration,
and [the publication pipeline](../usage/publication-pipeline.md) for collection,
assembly, planning, and rendering commands.
