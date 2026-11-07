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
