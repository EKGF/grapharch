---
title: RDF URL Collector
parent: Collectors
nav_order: 2
permalink: /collectors/rdf-urls/
grapharch:
  subject: urn:grapharch:docs:collectors:rdf-urls
---

# RDF URL Collector

The RDF URL Collector fetches an explicitly configured HTTP or HTTPS RDF
document, parses it in the declared format, and retains its assertions with
source provenance in the documentation KG. Use it when a vocabulary publisher
provides an RDF download URL rather than a repository checkout or a query service.

## What it documents

A downloaded RDF document can supply [ontology resources](../documentable-things/ontologies.md),
[SHACL shapes](../documentable-things/shacl-shapes.md),
[SKOS vocabularies](../usage/skos-taxonomies.md), or
[EKGF use cases](../usage/use-case-trees.md). The document's assertions determine
what it describes; the URL's filename does not establish its semantics.

## Example: collect a published ontology

Add a collector to the project in a website's publication definitions:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .

<urn:example:docs> ga:collector <urn:example:commons-roles> .

<urn:example:commons-roles> a ga:Collector ;
    ga:kind "rdf-url" ;
    ga:source "https://www.omg.org/spec/Commons/RolesAndCompositions/" ;
    ga:format "application/rdf+xml" .
```

This is a collector configuration fragment, not a complete publication project.
The surrounding definitions must supply the project, scope, profile and target.
The configured collector runs during the [website build](../usage/self-hosting.md);
publication scope determines which collected terms become documentation.
There is no separate `publication collect --kind rdf-url` CLI option.

The example URL serves the publisher's RDF/XML document. `ga:source` is a URL
**string literal**; `ga:format` explicitly supplies the RDF media type. Supported
formats include `text/turtle`, `application/rdf+xml`, `application/n-triples`,
`application/n-quads`, `application/trig`, `text/n3` and `application/ld+json`.
The response's content type must identify the same RDF format. Malformed RDF,
unsupported formats and responses above the 8 MiB source limit fail explicitly.
Relative IRIs resolve against the fetched URL.

## An RDF download is different from an endpoint query

The [SPARQL Endpoint Collector](sparql-endpoints.md) executes an explicit
`CONSTRUCT` or `DESCRIBE` request and collects the returned graph. This collector
instead reads the RDF document at the configured URL; it does not send a SPARQL
query. It also does not automatically fetch the document's `owl:imports` targets.
Register additional sources explicitly when their definitions are needed.

For a file maintained in Git, use a [configured Git RDF source](repositories.md#collect-one-rdf-file-from-git)
to retain repository, commit and blob provenance. For a local file, use the
[RDF File Collector](rdf-files.md).
