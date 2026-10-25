---
title: Usage
layout: default
nav_order: 1
grapharch:
  subject: urn:grapharch:docs:topic:usage
permalink: /usage/
---

# Using Grapharch

Grapharch is one Rust CLI with specialized collectors and renderers. The
`publication` commands operate on the shared documentation dataset.

Begin with [Getting started](getting-started.md), then run one of these workflows:

| Task | Guide |
|---|---|
| Collect sources and publish from a pinned KG | [Publication pipeline](publication-pipeline.md) |
| Generate this documentation website | [Self-hosted documentation](self-hosting.md) |
| Publish a standards-style specification and PDF | [Specification renderer](specification-renderer.md) |

The older `generate` command is a prototype. New work uses the graph-backed
publication pipeline. Distributed release binaries and a complete project
configuration system remain delivery work; build from source for now.
