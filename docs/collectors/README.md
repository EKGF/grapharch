---
title: Collectors
nav_order: 5
permalink: /collectors/
grapharch:
  subject: urn:grapharch:docs:collectors:readme
---

# Collectors

A collector reads source content into GraphArch's documentation graph. Choose
one for the input you have: a Markdown file, an RDF file, a SPARQL endpoint,
or another supported source. The table below shows which documentable things
each collector can supply.

Documentation generation turns collected content into reference entries,
explanations, and diagrams. Renderers present that content as websites or PDFs.

## Choose a collector

| Collector | Input | Documentable things it supports |
|---|---|---|
| [RDF File Collector](rdf-files.md) | Turtle, RDF/XML and other supported RDF serializations | Ontology resources, families and modules, SHACL shapes, SKOS vocabularies, EKGF use cases, publication hints |
| [RDF URL Collector](rdf-urls.md) | An explicit HTTP(S) RDF document URL and declared media type | Ontology resources, SHACL shapes, SKOS vocabularies, EKGF use cases and publication hints |
| [Markdown File Collector](markdown-files.md) | Markdown with optional YAML frontmatter | Pages, guides, sections, fenced examples, supported images and Mermaid flowcharts |
| [SPARQL File Collector](sparql-files.md) | Authored SPARQL text | [Query or statement fragments](../documentable-things/sparql-queries.md); the query is not executed |
| [SPARQL Endpoint Collector](sparql-endpoints.md) | An explicit CONSTRUCT or DESCRIBE response | The same RDF-defined things as an RDF file, with query-result provenance |
| [Published W3C Specification Collector](published-specifications.md) | A saved, published W3C HTML document | Supported specification sections, examples, references, and publication metadata |
| [Legal Preamble Collector](legal-preambles.md) | A selected legal disclosure in HTML | [Legal preambles](../documentable-things/legal-preambles.md) explicitly attached to publication targets |
| [Image Collector](images.md) | Supported PNG, JPEG, or SVG images | Retained illustrations and product logos |
| [Code Example Collector](code-examples.md) | An explicitly selected code file | Code examples published without execution |
| [Text Download Collector](text-downloads.md) | UTF-8 download files | Source-relative website downloads |
| [Repository Publication Collector](repository-publications.md) | GitHub pull requests and Vercel deployment metadata | Publication version-selection pages and preserved specification routes |
| [PlantUML Diagrams](plantuml.md) | An authored PlantUML file and bounded local include closure | Retained SVG illustrations with source and renderer provenance |
| [Abbreviation Files](abbreviation-files.md) | An authored Markdown abbreviation dictionary | Source-owned SKOS meanings and retained dictionary provenance |
| [Publication People](people.md) | ORCID iDs in collected sources and GitHub commit authors, through their public APIs | Editors and contributors in specification and PDF headers |

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

Collector additions can be proposed through pull requests following the
[contribution guide](../other/contribute/README.md) and the extension contract
above. Include source and provenance handling, bounded-input regression tests,
and a source-free rendering acceptance example.

## Collector definitions and configured uses

Collector definitions have canonical RDF identities, such as
`ga:RdfFileCollector`, and are instances of `ga:Collector`. A source-bound
configuration is a separate `ga:CollectorUse`, linked through `ga:collectorType`:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
<urn:my-project> ga:collector <urn:my-source> .
<urn:my-source> a ga:CollectorUse ;
    ga:collectorType ga:RdfFileCollector ;
    ga:source "ontology.ttl" .
```

Several uses can select the same definition while retaining different sources,
configuration, and provenance. The selector must be a supported canonical IRI;
string selectors and unknown collector identities fail validation. A configured
use selects an existing adapter; declaring a new RDF individual does not install
an executable adapter.
