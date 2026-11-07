---
title: Introduction
grapharch:
  subject: urn:mixed:introduction
---
Authored guidance from a Markdown file explains how to read this ontology.

The [data product definition](urn:mixed:DataProduct) is generated from OWL.
The [example query](urn:mixed:query) is collected as source text without execution.

## Choose an output

The same collected guidance can serve different readers without maintaining a
second copy of the prose.[^revision]

=== "Read on the web"

    Follow links between the Markdown introduction, the OWL class definition
    and the SPARQL example. The sources remain distinct, while the publication
    provides one reading path.

=== "Review a PDF"

    Download the review edition for a meeting or an offline reading session.
    Both tabs are included as labelled sections in the PDF, so no guidance is
    hidden behind an interaction that is unavailable on paper.

!!! note "Retain the original sources"

    The example query is documentation, not a build step. GraphArch retains its
    text and highlights the syntax without executing the query.

| Source | What it contributes |
| --- | --- |
| Markdown | Authored explanations, tabs, callouts and this table |
| OWL | The data product class and its definition |
| SPARQL | A readable query example, never executed during publication |

[^revision]: One collected revision supplies both editions, including this footnote. The exported documentation graph can be rendered after the original files are removed.
