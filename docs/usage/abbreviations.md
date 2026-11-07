---
title: Abbreviations
layout: default
parent: Usage
permalink: /usage/abbreviations/
grapharch:
  subject: urn:grapharch:docs:topic:usage:abbreviations
---
# Abbreviations

A project can select source-owned abbreviation dictionaries. Their meanings
are retained as SKOS concepts in the publication KG, so HTML, PDF, and search
use the same pinned content after the original source files are removed.

An RDF source can declare the dictionary directly:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .

<urn:project> ga:abbreviationScheme <urn:dictionary> .
<urn:dictionary> a skos:ConceptScheme .
<urn:dictionary:ekg> a skos:Concept ;
    skos:inScheme <urn:dictionary> ;
    skos:notation "EKG" ;
    skos:definition "Enterprise Knowledge Graph" .
```

Collect this source with the existing `rdf-file` collector. HTML wraps matching
whole terms in an `abbr` element whose title contains the authored meaning.
Matching is case-sensitive; `EKG` does not alter `EKGs`. Emphasis and links keep
their normal behavior. Code examples and image alternative text remain literal.
Definitions are escaped as text, never interpreted as HTML.

PDF keeps the authored short terms in the prose. Included Markdown dictionary
lines such as `*[EKG]: Enterprise Knowledge Graph` are removed from HTML, PDF,
and search presentation only when they match the selected dictionary. The
original source and retained fragment remain unchanged. Missing or conflicting
included definitions are errors. Without a selected dictionary, ordinary
Markdown behavior is unchanged.

A publication accepts at most eight schemes, 1,024 distinct abbreviations, and
128 KiB of dictionary text. Conflicting meanings for the same short term fail
publication; GraphArch does not choose a meaning based on collection order.

## Keep the dictionary in Markdown

An existing abbreviation file can remain the canonical source. Select it from
the project configuration; the path is relative to the project root and can sit
outside its content directory:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .

<urn:project> ga:collector <urn:project:abbreviations> .
<urn:project:abbreviations> a ga:Collector ;
    ga:kind "abbreviation-file" ;
    ga:source "abbreviations.md" .
```

```markdown
[//]: # (Source-owned shared abbreviations)
*[EKG]: Enterprise Knowledge Graph
*[API]: Application Programming Interface
```

Each nonblank line declares one abbreviation or a `[//]: #` comment. The
collector retains the exact original file, including comments and line endings,
with its source revision. It derives a SKOS scheme and concepts and selects the
scheme through `ga:abbreviationScheme`; no duplicate RDF dictionary is needed.

Definitions are plain text. Repeated terms are errors even when both meanings
are identical: correct duplicates in the canonical file. Errors identify the
source path and line. Malformed syntax, invalid UTF-8, empty dictionaries,
escaping paths, files larger than 128 KiB, more than 1,024 entries, terms longer
than 128 bytes, and definitions longer than 4,096 bytes stop collection.
