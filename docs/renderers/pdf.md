---
title: PDF
parent: Renderers
nav_order: 5
permalink: /renderers/pdf/
grapharch:
  subject: urn:grapharch:docs:renderers:pdf
---

# PDF renderer

A review or approval document with its own reading order.

## Output

A PDF compiled by the embedded Typst engine, with editable Typst source. An external Typst executable is not required.

## Select this renderer

Set the renderer on a publication target in your existing project configuration:

```turtle
<urn:example:target> ga:renderer "typst" .
```

This is a target setting, not a complete project definition. The target also needs
its scope, profile, destination, and any metadata required by its publication
style. A target can reuse content selected from the same graph as other targets.

See [PDF presentation](../features/targets/typst.md) for supported presentation and configuration,
and [the publication pipeline](../usage/publication-pipeline.md) for collection,
assembly, planning, and rendering commands.
