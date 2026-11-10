---
title: Bookshelf
parent: Renderers
nav_order: 4
permalink: /renderers/bookshelf/
grapharch:
  subject: urn:grapharch:docs:renderers:bookshelf
---

# Bookshelf renderer

An ontology family arranged into declared module books.

## Output

A catalogue of books with individual HTML specifications and PDFs. Book boundaries come from the publication model, not guessed file groupings.

## Select this renderer

Set the renderer on a publication target in your existing project configuration:

```turtle
<urn:example:target> ga:renderer <https://ekgf.org/ontology/grapharch#BookshelfRenderer> .
```

This is a target setting, not a complete project definition. The target also needs
its scope, profile, destination, and any metadata required by its publication
style. A target can reuse content selected from the same graph as other targets.

See [Bookshelves and module books](../usage/bookshelves.md) for supported presentation and configuration,
and [the publication pipeline](../usage/publication-pipeline.md) for collection,
assembly, planning, and rendering commands.
