---
title: Usage
layout: default
nav_order: 1
---
# Usage

GraphArch can be used in two primary modes:

1. **Ontology Documentation Mode**:
   To generate documentation for ontologies or schemas, use the following command:

   ```bash
   cargo run GraphArch --mode ontology --input <file|url>
   ```

2. **Graph Endpoint Discovery Mode**:
   To connect to a graph database endpoint and generate reports:

   ```bash
   cargo run GraphArch --mode graph --url <endpoint>
   ```

> [!NOTE]
> The GraphArch tool is currently under active development and may undergo
> significant changes. The plan is to create a downloadable binary app that
> people can easily install and run. The commands above are needlessly verbose,
> used for development purposes only.
