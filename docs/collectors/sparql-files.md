---
title: SPARQL File Collector
parent: Collectors
nav_order: 4
permalink: /collectors/sparql-files/
grapharch:
  subject: urn:grapharch:docs:collectors:sparql-files
---

# SPARQL File Collector

The SPARQL-file collector retains a query or statement as documentation. It does
not run that statement, contact an endpoint, or document its result rows. Use the
[endpoint collector](sparql-endpoints.md) when the desired source is RDF returned
by an endpoint.

## What it documents

It documents [SPARQL query and statement fragments](../documentable-things/sparql-queries.md)
associated with a documentable subject.
Such fragments can accompany [pages and guides](../documentable-things/authored-documentation.md)
or explain queries over [ontology resources](../documentable-things/ontologies.md).
The retained format is `application/sparql-query`.

## Retain a query example

Save `products.rq`:

```sparql
# grapharch:subject urn:example:product-query
PREFIX ex: <https://example.org/terms#>
SELECT ?product WHERE { ?product a ex:Product }
```

The pragma is written exactly as shown: `# grapharch:subject` followed by the
subject IRI. Unsupported documentation pragmas and multiple subject pragmas are
errors.

```sh
grapharch publication collect --store output/kg \
  --project urn:example:project --collector urn:example:product-query \
  --kind sparql --source products.rq
```

The library adapter is `SparqlFile`. Assembly and publication planning still need
a scope and profile that include the fragment; collecting it alone produces no
website. See [the staged pipeline](../usage/publication-pipeline.md).
