---
title: Collectors
nav_order: 5
permalink: /collectors/
grapharch:
  subject: urn:grapharch:docs:collectors:readme
---

# Collectors

A collector capability reads one specific kind of input and retains its content, hints, and
provenance in the documentation KG. A build creates separate instances for
individual files or queries; the RDF File Collector is one capability, not a
different collector kind for each RDF ontology or each file. Local files and Git
blobs use transport adapters for that capability. Documentation generators then describe the things in that
collected graph. Renderers publish an assembled revision; collectors do not pick
a website layout or a chapter order.

## Choose a collector

| Collector | Input | Documentable things it supports |
|---|---|---|
| [RDF File Collector](rdf-files.md) | Turtle, RDF/XML and other supported RDF serializations | Ontology resources, families and modules, SHACL shapes, SKOS vocabularies, EKGF use cases, publication hints |
| [Markdown File Collector](markdown-files.md) | Markdown with optional YAML frontmatter | Pages, guides, sections, fenced examples, supported images and Mermaid flowcharts |
| [SPARQL File Collector](sparql-files.md) | Authored SPARQL text | [Query or statement fragments](../documentable-things/sparql-queries.md); the query is not executed |
| [SPARQL Endpoint Collector](sparql-endpoints.md) | An explicit CONSTRUCT or DESCRIBE response | The same RDF-defined things as an RDF file, with query-result provenance |
| [Published W3C Specification Collector](published-specifications.md) | A saved, published W3C HTML document | Supported specification sections, examples, references, and publication metadata |
| [Legal Preamble Collector](legal-preambles.md) | A selected legal disclosure in HTML | [Legal preambles](../documentable-things/legal-preambles.md) explicitly attached to publication targets |
| [Image Collector](assets-and-code.md#image-collector) | Supported PNG, JPEG, or SVG images | Retained illustrations and product logos |
| [Code Example Collector](assets-and-code.md#code-example-collector) | An explicitly selected code file | Code examples published without execution |
| [Text Download Collector](assets-and-code.md#text-download-collector) | UTF-8 download files | Source-relative website downloads |

[Repository Scanners](repositories.md) discover local files or pinned Git blobs
and dispatch the matching file collectors. They provide discovery and source
provenance rather than an additional kind of semantic recognition.

See [Documentable Things](../documentable-things/README.md) for recognition rules
and concrete source examples for each kind of thing. Repository transport does
not itself recognise a class, taxonomy, or use case: RDF collectors retain the
assertions, and the corresponding documentation generators interpret their explicit types and
relationships.

## Collection is not always automatic

Directory and repository family builds discover supported RDF files. Website
builds discover Markdown pages and selected downloads. The staged CLI exposes
individual RDF, Markdown, SPARQL-file, SPARQL-endpoint, and published-W3C
collectors. Other collectors are called by specific publishing workflows or the
library API; they do not each have a standalone CLI flag. Each page states its
entry point.

Independent collectors stream bounded contributions into a shared graph writer.
A failure is recorded against its source; configured error policy determines
whether independent work continues. A failed contribution cannot silently replace
valid source content. See [the publication pipeline](../usage/publication-pipeline.md)
and [quality review](../usage/quality-review.md).

## Add a specialised collector

New source kinds can add small collectors without expanding one catch-all
parser. A collector must retain stable identities, source revisions, and
provenance, obey resource budgets, and support rendering after the original source
has been removed. A new RDF resource kind may instead need a documentation generator over
existing RDF collection. See [extensibility](../features/extensibility.md) and the
[architecture](../other/architecture/README.md).
