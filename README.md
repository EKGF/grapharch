# GraphArch

**GraphArch** is an open-source tool developed
by the Enterprise Knowledge Graph Forum ([EKGF](ekgf.org)),
a Managed Community of the
Object Management Group ([OMG](https://www.omg.org/communities/enterprise-knowledge-graph-forum.htm)).
The tool aims to generate comprehensive
documentation for knowledge graphs, shape graphs,
labelled property graphs, semantic graphs,
or ontologies,
with outputs available as both websites
(markdown) and
PDFs ([typst](https://typst.app/docs/)).
This project is part of an effort to
streamline graph documentation and governance,
enhancing clarity and accessibility for
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

- **Ontology Precision**:
  GraphArch emphasizes governance and precision in documenting ontologies,
  schemas, and their relationships. It aims to deliver librarian-level
  accuracy of definitions, domain ownership, and control for enterprise data management.
- **Business-Oriented Profiling**:
  GraphArch offers capabilities for graph database endpoint analysis that
  can be particularly useful for business users to understand and describe
  their data in their own terms.
- **Flexible Output Formats**:
  Documentation can be generated as user-friendly websites or professionally
  formatted PDF reports.
- **Onboard Graph Engine**:
  The tool includes [OxiGraph](https://github.com/oxigraph/oxigraph),
  a graph database that can be used to generate data for further documentation,
  integrating both input options seamlessly.

For all features see [docs/feature/README.md](docs/feature/README.md).

## Getting Started

This repository will serve as the foundation for GraphArch's ongoing development.
Contributions are welcome to help grow this initiative!

- Get started as contributor on [MacOS](docs/contribute/macos.md)
- TODO: Linux, Windows

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

## Contributing

We welcome contributions from the community!
Please refer to our [Contributing Guidelines](CONTRIBUTING.md)
for details on how to get involved.

## License

GraphArch is released under the [MIT License](LICENSE).

## Contact

For more information about GraphArch or the Enterprise Knowledge Graph Forum,
please contact us at [info@ekgf.org](mailto:info@ekgf.org).

---

Join us in building a comprehensive tool for documenting and exploring the world of enterprise graphs!
