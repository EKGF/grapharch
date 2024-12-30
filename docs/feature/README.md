---
title: Features
layout: default
nav_order: 2
---

# GraphArch Features

TODO: Just a few examples of features, there's so much more

## General

- [Ontology Precision](./general-ontology-precision.md)
- [Business Oriented Profiling](./general-business-oriented-profiling.md)
- [Flexible Output Formats](./general-flexible-output-formats.md)
- [Onboard Graph Engine](./general-onboard-graph-engine.md)

## Sources & Formats

Things to document can come from any number of sources providing
data in a variety of formats and structures.

1. [RDF File](./source-file-rdf.md)
1. [Markdown File](./source-file-markdown.md)
1. [git](./source-endpoint-git-repo.md)
1. [SPARQL](./source-endpoint-sparql.md)
1. [Cypher](./source-endpoint-cypher.md)
1. [GSQL](./source--endpointgsql.md)
1. [GraphQL](./source-endpoint-graphql.md)
1. [SQL](./source-endpoint-sql.md)
1. ...

Obviously, the primary focus is on RDF data, either retrieved
via SPARQL queries or loaded from files in formats like
[Turtle](https://www.w3.org/TR/turtle/),
[RDF/XML](https://www.w3.org/TR/rdf-xml/),
[N-Triples](https://www.w3.org/TR/n-triples/),
[JSON-LD](https://www.w3.org/TR/json-ld/),
etc.

But other kinds of data could be supported as well such as Markdown,
JSON, CSV, XML, YAML, TOML, Parquet, again there's no limit, as long as someone writes a plugin for it to support it.

1. [Markdown](./source-file-markdown.md)
1. [JSON](./source-file-json.md)
1. [CSV](./source-file-csv.md)
1. [XML](./source-file-xml.md)
1. [YAML](./source-file-yaml.md)
1. [TOML](./source-file-toml.md)

## Targets

The output of the GraphArch process can be written to any number of
targets, for instance:

### HTML

- [markdown (.md) files for MkDocs](./target-markdown-for-mkdocs)
- [reStructuredText (.rst) files for sphinx](./target-rst-for-sphinx)

### PDF

- [typst](./target-typst.md)

### Other

- Any other documentation system, for instance Confluence could be updated etc
