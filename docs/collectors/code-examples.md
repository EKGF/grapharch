---
title: Code Example
parent: Collectors
nav_order: 9
permalink: /collectors/code-examples/
grapharch:
  subject: urn:grapharch:docs:collectors:code-examples
---

# Code Example Collector

The Code Example Collector retains an explicitly selected code file as a
documentation fragment. It preserves the source text without executing it or
fetching its dependencies.

## What it documents

Code examples accompany [pages and guides](../documentable-things/authored-documentation.md)
and [published specifications](../documentable-things/published-specifications.md).
Supported formats are JSON-LD, JSON, Turtle, SPARQL, and plain text. Retaining a
JSON example does not interpret arbitrary JSON records as documentable things.

## Configure a code example

The DPROD showcase's source-owned publication definitions demonstrate explicit
code jobs. A job identifies the file, its subject, its fragment, and its format:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix ex: <urn:example:> .

ex:project ga:collector ex:query-example-source .
ex:query-example-source a ga:CollectorUse ; ga:collectorType ga:CodeFileCollector ;
    ga:source "examples/products.rq" ; ga:sourcePath "examples/products.rq" ;
    ga:fragment ex:query-example ; ga:subject ex:product-query ;
    ga:format "application/sparql-query" .
```

Use one of `application/ld+json`, `application/json`, `text/turtle`,
`application/sparql-query`, or `text/plain`. This is the configured publishing
workflow shown in the
[DPROD example](https://github.com/EKGF/dprod/blob/develop/publication/standards.ttl),
not a standalone `--kind code` CLI command. Publication definitions still choose
where the fragment appears.

For code fenced inside Markdown, use the
[Markdown File Collector](markdown-files.md). For a query file with documentation
pragmas, use the [SPARQL File Collector](sparql-files.md).
