---
title: "GraphArch Documentation"
layout: home
nav_exclude: true
---

# GraphArch Documentation

GraphArch is a tool that generates documentation from various types of sources.
It's primary focus is on generating documentation from graph databases, in particular
OWL ontologies and SHACL shapes.

[![.github/workflows/rust-ci.yml](https://github.com/EKGF/grapharch/actions/workflows/rust-ci.yml/badge.svg?branch=main)](https://github.com/EKGF/grapharch/actions/workflows/rust-ci.yml) [![CLA assistant](https://cla-assistant.io/readme/badge/EKGF/grapharch)](https://cla-assistant.io/EKGF/grapharch)

**GraphArch** is an open-source tool developed
by the Enterprise Knowledge Graph Forum ([EKGF](https://ekgf.org)),
a Managed Community of the
Object Management Group ([OMG](https://www.omg.org/communities/enterprise-knowledge-graph-forum.htm)).

The tool aims to generate comprehensive
documentation for knowledge graphs, shape graphs,
labelled property graphs, semantic graphs, ontologies,
taxonomies, use cases, and more,
with outputs available as both websites (markdown) and
PDFs (see [Targets](features/targets/README.md)).

This project is part of an effort to streamline graph documentation
and governance, enhancing clarity and accessibility for
various stakeholders.

## Project Overview

GraphArch serves two primary use cases:

1. **Ontology and Schema Documentation**

   GraphArch can document ontologies,
   whether OWL-based or non-semantic tech,
   such as LPG graph schemas from platforms
   like Neo4j or TigerGraph.
   This includes individual ontologies as
   well as ontology families,
   where multiple related ontologies might
   be documented collectively, such as the
   Financial Industry Business Ontology (FIBO).

2. **Graph Database Endpoint or EKG Documentation**

   GraphArch can connect to graph database
   endpoints (SPARQL, Cypher, GSQL, etc.)
   and generate documentation through discovery.
   It analyzes the data structures, provides
   profiling, and creates reports based on the
   detected entities.
   This could include a breakdown of the classes,
   distribution of instances, identification of PII,
   and a summary of data models used,
   generating outputs like dashboards or reports.

## Key Features

- [**Ontology Precision**](features/general/ontology-precision.md):
  GraphArch emphasizes governance and precision in documenting ontologies,
  schemas, and their relationships. It aims to deliver librarian-level
  accuracy of definitions, domain ownership, and control for enterprise data management.
- [**Business-Oriented Profiling**](features/general/business-oriented-profiling.md):
  GraphArch offers capabilities for graph database endpoint analysis that
  can be particularly useful for business users to understand and describe
  their data in their own terms.
- [**Flexible Output Formats**](features/general/flexible-output-formats.md):
  Documentation can be generated as user-friendly websites or professionally
  formatted PDF reports.
- [**Onboard Graph Engine**](features/general/onboard-graph-engine.md):
  The tool includes [OxiGraph](https://github.com/oxigraph/oxigraph),
  a graph database that can be used to generate data for further documentation,
  integrating both input options seamlessly.

For all features see [Features](features/README.md).

## Getting Started

- [ ] Download & Install GraphArch
- [ ] Run GraphArch

> [!NOTE]
> Downloading and installing GraphArch is not yet supported,
> unless you are a developer and willing to build it from source.
> See [Contributing to GraphArch](contribute/) for more details.

### Usage

GraphArch can be used in two primary modes:

1. **Ontology Documentation Mode**:
   To generate documentation for ontologies or schemas, use the following command:

   ```bash
   cargo run grapharch --mode ontology --input <file|url>
   ```

2. **Graph Endpoint Discovery Mode**:
   To connect to a graph database endpoint and generate reports:

   ```bash
   cargo run grapharch --mode graph --url <endpoint>
   ```

> [!NOTE]
> The GraphArch tool is currently under active development and may undergo
> significant changes. The plan is to create a downloadable binary app that
> people can easily install and run. The commands above are needlessly verbose,
> used for development purposes only.

## Contributing

We welcome contributions from the community!

This repository will serve as the foundation for GraphArch's ongoing development.

See [How to Contribute](contribute/README.md) for more details.

## License

GraphArch is released under the [MIT License](./LICENSE.md).

## Contact

For more information about GraphArch or the Enterprise Knowledge Graph Forum,
please contact us at [info@ekgf.org](mailto:info@ekgf.org).

---

Join us in building a comprehensive tool for documenting and exploring the world of enterprise graphs!

See [Design Goals](design-goals/README.md) and [Features](features/README.md) for more information.
