# EKGF ontology family

## Published repository showcase

The published gallery uses [`showcase.yaml`](showcase.yaml), which selects
`https://github.com/EKGF/ekg-ontologies.git` at `ref: HEAD`. Every build fetches
the repository's latest default branch into a reusable bare Git cache, then
collects the resolved commit. It does not read the copied `sources/` corpus.

The observed commit, repository URL, authors and committers are recorded in the
downloadable documentation KG. A moving branch therefore produces a publication
pinned to the concrete commit seen by that build. Use a full commit ID instead
of `HEAD` in the same `ref` field when a showcase needs a reproducible historical
revision. An unavailable repository or revision fails the build; cached or
copied content is never silently substituted.

The configuration declares the definitions file, project and publication target,
so the same family showcase pipeline works with another ontology repository.
Its RDF collectors generate a family reference, PDF and cross-ontology links.
Missing English labels and definitions become explicit KG diagnostics. No
source descriptions are invented, imports fetched, or SHACL validation or OWL
reasoning implied.

To build the repository example directly:

```sh
grapharch publication build-family \
  --git-repository https://github.com/EKGF/ekg-ontologies.git --git-ref HEAD \
  --definitions examples/ekg-ontologies/project.ttl \
  --project urn:ekgf-family:project --target urn:ekgf-family:specification \
  --output-root output/ekgf-family
```

## Separate offline acceptance corpus

The checked-in CC0 `sources/` directory captures the `EKGF/ekg-ontologies`
working tree after its October 2026 documentation pass, based on commit
`ce7b810990a806333d698e2934c38c8a1384b2ec`. `reference.yaml` records the original
uncommitted state and SHA-256 digests. It contains 24 ontologies and 704 named
resources with English labels and definitions; DPROD and GraphArch are separate
projects.

This corpus supports deterministic offline acceptance tests. The gallery test
explicitly builds a local bare repository from it and injects that repository
into its test configuration. Repository tests cover refreshed default branches,
immutable commits, cache reuse and hard failures without a corpus fallback.
Rendering is also checked after deleting sources and restoring the exported KG.
