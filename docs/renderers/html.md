---
title: Single-document HTML
parent: Renderers
nav_order: 6
permalink: /renderers/html/
grapharch:
  subject: urn:grapharch:docs:renderers:html
---

# Single-document HTML renderer

A compact standalone document with a contents list.

## Output

One HTML document, its stylesheet, retained illustrations, and provenance. Use the website renderer for a multi-page companion site.

## Select this renderer

Set the renderer on a publication target in your existing project configuration:

```turtle
<urn:example:target> ga:renderer <https://ekgf.org/ontology/grapharch#HtmlRenderer> .
```

This is a target setting, not a complete project definition. The target also needs
its scope, profile, destination, and any metadata required by its publication
style. A target can reuse content selected from the same graph as other targets.

See [HTML presentation](../features/targets/html.md) for supported presentation and configuration,
and [the publication pipeline](../usage/publication-pipeline.md) for collection,
assembly, planning, and rendering commands.
