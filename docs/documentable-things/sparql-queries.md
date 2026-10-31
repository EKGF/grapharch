---
title: SPARQL queries
parent: Documentable Things
nav_order: 8
permalink: /documentable-things/sparql-queries/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:sparql-queries
---

# SPARQL queries

A SPARQL query can be documentation in its own right: an example of using a
vocabulary, a reusable query for a guide, or a statement that reviewers need to
inspect. GraphArch can retain its source text as a documentation fragment and
include that fragment in HTML and PDF publications.

## Example: document a query

The mixed-sources showcase contains this complete `query.rq` file:

```sparql
# grapharch:subject urn:mixed:query
PREFIX owl: <http://www.w3.org/2002/07/owl#>
SELECT ?product WHERE {
    ?product a owl:Class .
}
```

The `# grapharch:subject` pragma associates the fragment with a documentable
subject. In this example the subject is `urn:mixed:query`; the publication scope
includes that subject and the profile places its fragment under **Example
query**. The text remains query source, rather than becoming a table of results.

Compare the [query source](https://github.com/EKGF/grapharch/blob/main/examples/mixed-sources/query.rq)
and [publication definitions](https://github.com/EKGF/grapharch/blob/main/examples/mixed-sources/project.ttl)
with **Example query** in the
[generated guide](https://ekgf.github.io/grapharch/showcases/artifacts/mixed/website/index.html)
and [review PDF](https://ekgf.github.io/grapharch/showcases/artifacts/mixed/review/document.pdf).

## Supporting collector and recognition

The [SPARQL file collector](../collectors/sparql-files.md) reads a configured
query file and retains the complete source with provenance, using the format
`application/sparql-query`. The subject pragma is optional; when supplied it must
contain a valid IRI. Multiple subject pragmas and unsupported `grapharch` pragmas
fail explicitly. Collection does not execute or validate the query against an
endpoint. A scope and profile must select the fragment before it appears in a
publication.

For example, collect the file into an existing publication project:

```sh
grapharch publication collect --store output/kg \
  --project urn:mixed:project --collector urn:mixed:sparql \
  --kind sparql --source examples/mixed-sources/query.rq
```

This collects documentation; it does not publish the guide by itself. See the
[publication pipeline](../usage/publication-pipeline.md) for assembly and target
planning.

## Query text versus endpoint results

The [SPARQL endpoint collector](../collectors/sparql-endpoints.md) serves a
different purpose: it executes a configured `CONSTRUCT` or `DESCRIBE` request to
collect returned RDF as source material. Use that collector to document
ontologies or shapes stored at an endpoint. Use the SPARQL file collector when
the query itself is what readers should see.
