---
title: Getting Started
layout: default
parent: Usage
nav_order: 1
grapharch:
  subject: urn:grapharch:docs:topic:usage:getting-started
permalink: /usage/getting-started/
---

# Getting started

Build Grapharch from a repository checkout with the Rust toolchain specified in
`rust-toolchain.toml`:

```sh
cargo build
cargo run -- publication --help
```

## Generate this website

```sh
cargo run -- publication build-site \
  --source docs \
  --definitions docs/grapharch.ttl \
  --project urn:grapharch:docs \
  --target urn:grapharch:docs:website \
  --output-root output/docs
python3 -m http.server 8770 --directory output/docs/site
```

Open `http://localhost:8770`. The [self-hosting guide](self-hosting.md) explains
the graph model and how to edit documentation fragments.

## Run the showcases

```sh
cargo run --example publish_showcase
cargo run --example publish_showcase -- --mixed
cargo run --example publish_showcase -- --specification
```

These exercise authored RDF, mixed Markdown/SPARQL/OWL, and a formal specification.
Generated artifacts stay under the ignored `output/` directory.

## Explore individual stages

The [publication pipeline guide](publication-pipeline.md) documents independent
collection, assembly, planning, rendering, querying, and dataset export/import.
Read the [status page](../other/status.md) before relying on a planned feature.
