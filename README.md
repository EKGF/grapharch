# Grapharch

Graph-backed documentation: collect authored text and RDF constructs into a
knowledge graph, then publish websites, specifications, and PDF documents.

Grapharch generates its own documentation site:

```sh
cargo run -- publication build-site \
  --source docs --definitions docs/grapharch.ttl \
  --project urn:grapharch:docs --target urn:grapharch:docs:website \
  --output-root output/docs
python3 -m http.server 8770 --directory output/docs/site
```

See [Getting started](docs/usage/getting-started.md),
[Self-hosting](docs/usage/self-hosting.md), and
[Implementation status](docs/other/status.md).
