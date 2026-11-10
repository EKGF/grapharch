---
title: Website
parent: Renderers
nav_order: 1
permalink: /renderers/website/
grapharch:
  subject: urn:grapharch:docs:renderers:website
---

# Website renderer

A static companion website for guides, explanations, and reference pages.

## Output

HTML pages with topic navigation, search, retained assets, and provenance.

## Select this renderer

Set the renderer on a publication target in your existing project configuration:

```turtle
<urn:example:target> ga:renderer "website" .
```

This is a target setting, not a complete project definition. The target also needs
its scope, profile, destination, and any metadata required by its publication
style. A target can reuse content selected from the same graph as other targets.

See [Project websites](../usage/project-websites.md) for supported presentation and configuration,
and [the publication pipeline](../usage/publication-pipeline.md) for collection,
assembly, planning, and rendering commands.
