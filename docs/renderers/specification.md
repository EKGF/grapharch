---
title: Specification
parent: Renderers
nav_order: 2
permalink: /renderers/specification/
grapharch:
  subject: urn:grapharch:docs:renderers:specification
---

# Specification renderer

A single standards-style reference with publication metadata and numbered sections.

## Output

Specification HTML, a matching PDF, and editable Typst source.

## Select this renderer

Set the renderer on a publication target in your existing project configuration:

```turtle
<urn:example:target> ga:renderer "specification" .
```

This is a target setting, not a complete project definition. The target also needs
its scope, profile, destination, and any metadata required by its publication
style. A target can reuse content selected from the same graph as other targets.

See [Specification setup](../usage/specification-renderer.md) for supported presentation and configuration,
and [the publication pipeline](../usage/publication-pipeline.md) for collection,
assembly, planning, and rendering commands.
