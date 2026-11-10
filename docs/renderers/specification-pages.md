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
