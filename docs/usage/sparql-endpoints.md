---
title: SPARQL endpoints
parent: Usage
nav_order: 8
permalink: /usage/sparql-endpoints/
grapharch:
  subject: urn:grapharch:docs:topic:sparql-endpoints
---

# Collect and publish with SPARQL services

GraphArch can collect ontologies, SHACL shapes, and other RDF from an existing
SPARQL service. The service providing source material and the service storing
the documentation KG are independent choices.

## Collect RDF from an endpoint

Create a source configuration, for example `ontology-source.yaml`:

```yaml
endpoint: https://data.example.org/sparql
query: |
  CONSTRUCT { ?subject ?predicate ?object }
  WHERE { GRAPH <https://example.org/ontology> { ?subject ?predicate ?object } }
max_source_bytes: 8388608
http:
  timeout_seconds: 30
  authentication:
    kind: bearer
    token_env: ONTOLOGY_API_TOKEN
```

```sh
grapharch publication collect --store output/kg \
  --project urn:example:project --collector urn:example:ontology-source \
  --kind sparql-endpoint --source ontology-source.yaml
```

Each configured collector makes one `CONSTRUCT` or `DESCRIBE` request and owns
one isolated source contribution. Use separate collector identities for distinct
queries against the same endpoint. Optional `default_graphs` and `named_graphs`
set protocol dataset parameters; `source_graph` names the graph for returned
triples. The response must identify its RDF serialization with `Content-Type`.
GraphArch checks response limits, computes its source revision, normalizes blank
nodes, and streams parsed assertions through its bounded graph writer. A bounded
temporary file allows stable revision-scoped blank-node identities without
holding the result graph in memory. Failed requests or malformed RDF do not
replace the last successful contribution and block new assembly.

To publish an ontology family directly from several endpoint queries, place a
list in `family-sources.yaml`, with each item containing `collector` and the
nested `source` configuration above. Run:

```sh
grapharch publication build-family --sparql-sources family-sources.yaml \
  --definitions project.ttl --project urn:example:project \
  --target urn:example:specification --output-root output/family
```

Source queries run concurrently with the configured collection concurrency.
GraphArch does not automatically fetch ontology imports or query other services.
An endpoint query produces an observed result revision; it does not claim that
independent queries all observed the source service at the same instant.

## Store the documentation KG in a service

A writable documentation KG needs stronger guarantees than ordinary SPARQL
Query and Update provide. GraphArch currently accepts a service implementing
RDF4J HTTP transactions with **verified SERIALIZABLE isolation**. A standard
query/update-only endpoint is a collection source, but cannot serve as this KG
backend.

Create `documentation-store.yaml`:

```yaml
query_endpoint: https://kg.example.org/rdf4j-server/repositories/documentation
transactions_endpoint: https://kg.example.org/rdf4j-server/repositories/documentation/transactions
http:
  timeout_seconds: 30
  max_response_bytes: 134217728
  authentication:
    kind: basic
    username_env: GRAPHARCH_KG_USERNAME
    password_env: GRAPHARCH_KG_PASSWORD
```

Use `--sparql-store documentation-store.yaml` instead of `--store output/kg`
for `collect`, `snapshot`, `assemble`, `resolve-scope`, `plan`, `render`, `query`,
`export`, and `import`. `build-family` also accepts either backend; omitting both
selects a fresh temporary in-memory KG. Its ownership manifest retires deleted
inputs belonging to that family build while preserving other contributors and
historical pinned revisions. Failed source discovery blocks promotion.
The all-in-one `build-site` command currently uses its own fresh in-memory KG.

On opening a remote KG, GraphArch requests serializable transactions and tests
for a phantom/write-skew conflict using two transactions with disjoint inserts
into a fresh probe graph. Both transactions first read the same empty graph;
both commits must not succeed. GraphArch checks that its query endpoint sees the
committed probe and removes that graph after verification. This requires write
permission even for commands that subsequently only read. The check is scoped
to a unique probe graph and never clears existing application graphs.

Contribution promotion, assembly, snapshots, and archive restoration retain
transactional isolation. A pinned revision references immutable contribution
graphs retained in the service; renderers read that revision rather than source
files or changing contribution heads. Exports preserve private control metadata
and named graph contexts, allowing restoration into an empty backend. Treat a
whole-KG export as an administrative archive, not a filtered public RDF download.

Transaction locations must remain within the configured repository's transaction
path and origin. Redirects are rejected. Timeouts, HTTP errors, serialization
conflicts, and response-size violations fail explicitly. GraphArch never retries
an uncertain commit: it stops using that client until the dataset is reopened
and its state inspected. Cancellation rolls back an uncommitted transaction;
a server must also expire abandoned transactions after a process crash.

Credentials come from environment variables and are never written into source
provenance. Endpoint URLs must not contain credentials, query strings, or
fragments. Use the operating system's trusted certificate authorities for HTTPS.

The protocol is described in the [RDF4J REST API](https://rdf4j.org/documentation/reference/rest-api/)
and [transaction isolation documentation](https://rdf4j.org/documentation/programming/repository/).
The SPARQL integration CI runs the complete family publish/export/restore path
against a dedicated NativeStore repository; deterministic local protocol tests
also exercise rejected isolation, failures, response limits, and invalid locations.
