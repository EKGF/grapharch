---
title: Local builds and CI
parent: Features
nav_order: 6
permalink: /features/workflow/
grapharch:
  subject: urn:grapharch:docs:topic:features:workflow
---

# Use the same publication workflow locally and in CI

Install the CLI, keep publication definitions with your project, and build into
a dedicated output directory. After editing source content or hints, run the
same command again. The generated site is ready for a static host such as
GitHub Pages.

## Choose a build path

| Project | Start with |
| --- | --- |
| A repository of related ontologies | [Ontology-family builds](../usage/ontology-families.md) |
| A tree of authored documentation | [Website builds](../usage/self-hosting.md) |
| Several independently arranged publications | [Publication targets](../usage/publication-families.md) |
| Explicit control over collection, planning, and rendering | [Staged publication commands](../usage/publication-pipeline.md) |

Use [Getting started](../usage/getting-started.md) for prebuilt installation
options and the [GitHub Actions guide](../usage/github-actions.md) for CI setup.
Pin a released GraphArch version in automation so a rebuild uses the intended
generator version.

## Rebuild without losing a good publication

Independent sources are collected concurrently. Persistent family builds can
reuse unchanged contributions, and bookshelf publications can reuse unchanged
book PDFs after checking their inputs and checksums. The Git cache avoids a
fresh clone on every remote build.

Generated files belong to their target. GraphArch stages a complete generation
before promoting it. Failed or superseded work does not replace the previous
complete publication, and files that GraphArch does not own are preserved.

## Keep failure policy explicit

The default is to stop on an execution error. `--keep-going` enables supported
diagnostic previews; it does not turn an incomplete release into an approved
standard. Quality warnings remain visible in the output and manifest.

Rebuild time depends on source size, queries, and PDF complexity. Large books
can take materially longer than an HTML-only reference. Continuous file
watching and a live editor extension are not current CLI features; local work
uses explicit rebuilds.
