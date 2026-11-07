---
title: PlantUML Diagrams
parent: Collectors
nav_order: 12
permalink: /collectors/plantuml/
grapharch:
  subject: urn:grapharch:docs:collectors:plantuml
---

# PlantUML diagrams

Keep authored diagrams and their local themes with the documentation. GraphArch
collects the source, resolves local includes, and retains the generated SVG in
the documentation graph. HTML and PDF publication then use that retained image.
An exported graph can publish the diagram after the source files and diagram
renderer have been removed.

## Configure a diagram

Install PlantUML and Graphviz on the build machine and confirm that
`plantuml -version` succeeds. The renderer must be available on `PATH`.

Declare each diagram explicitly in the project's publication definitions:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .

<urn:manual> a ga:Project ;
    ga:contentSource "docs" ;
    ga:collector <urn:manual:lifecycle-diagram> .

<urn:manual:lifecycle-diagram> a ga:Collector ;
    ga:kind "plantuml-file" ;
    ga:source "docs/diagrams/src/lifecycle.puml" ;
    ga:destination "docs/diagrams/out/lifecycle.svg" ;
    ga:rendererExecutable "plantuml" .
```

Both paths are relative to the project root. The destination must be beneath
the selected documentation content root. Reference that destination from Markdown:

```markdown
![Product lifecycle](diagrams/out/lifecycle.svg)
```

The destination declares an image identity; an existing generated SVG is not
required. Exclude old generated diagram directories from source discovery when
migrating an existing documentation project. Each destination has one producer.
Safe authored SVG `<object>` elements are normalized to retained Markdown images;
active object attributes and fallback HTML are rejected.

## Includes, provenance and limits

Local `!include` and `!include_once` directives resolve relative to their source
file, including project-owned themes. Every included file must remain beneath
the project root. GraphArch retains original file contents and the expanded
source separately, with source revision provenance. The retained asset provenance records the local
PlantUML version used to generate it. PlantUML's metadata processing instruction
is represented by that graph provenance and is removed from the published SVG.

A source file may contain one diagram. Its include closure is limited to
32 files, eight levels, 64 KiB per file and 128 KiB overall. Rendering has a
30-second deadline and bounded output. The generated SVG must pass the shared
[image validation and publication limits](../collectors/images.md).

The explicitly selected tool runs with PlantUML's
[SANDBOX security profile](https://plantuml.com/security) and a restricted
environment. Remote includes, dynamic include paths, implicit themes and
arbitrary source-selected commands are rejected. Renderer installation is an
explicit build prerequisite; a missing tool or invalid diagram fails the build.

On subsequent builds, unchanged source closures and the same PlantUML version
reuse the retained SVG. HTML and PDF rendering do not invoke PlantUML.
