---
title: Abbreviation Files
parent: Collectors
nav_order: 13
permalink: /collectors/abbreviation-files/
grapharch:
  subject: urn:grapharch:docs:collectors:abbreviation-files
---
# Abbreviation Files

The Abbreviation File Collector supports one documentable thing: an authored
Markdown abbreviation dictionary. It retains the original file and converts
its entries into source-owned [SKOS concepts](../usage/skos-taxonomies.md).

```markdown
[//]: # (Shared abbreviation meanings)
*[EKG]: Enterprise Knowledge Graph
*[API]: Application Programming Interface
```

Select the file from the project configuration:

```turtle
<urn:project> ga:collector <urn:project:abbreviations> .
<urn:project:abbreviations> a ga:Collector ;
    ga:kind "abbreviation-file" ;
    ga:source "abbreviations.md" .
```

The path is relative to the project root. The file may live outside the content
directory, but must remain inside the project. Every nonblank line is one
`*[TERM]: Definition` declaration or a `[//]: #` comment. Definitions are plain
text. Duplicate terms, including identical duplicates, are errors with line
numbers; GraphArch does not select a meaning according to source order.

The original UTF-8 bytes, comments, line endings, source path, and revision are
retained in the documentation graph. Derived concepts carry notation, preferred
label, definition, scheme membership, and source-fragment provenance. The
project selects the generated scheme through `ga:abbreviationScheme`.

The collector accepts at most 128 KiB, 1,024 entries, 128 bytes per term, and
4,096 bytes per definition. Invalid syntax, invalid UTF-8, empty dictionaries,
and paths escaping the project fail collection. HTML and PDF can render after
the source tree is deleted; renderers read the pinned graph.

See [abbreviation presentation](../usage/abbreviations.md) for HTML explanations,
code exclusions, PDF behavior, and direct RDF dictionaries.
