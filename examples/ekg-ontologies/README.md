# EKGF ontology family

This pinned CC0 corpus captures the `EKGF/ekg-ontologies` working tree after
its October 2026 documentation pass, based on commit
`ce7b810990a806333d698e2934c38c8a1384b2ec`. Native files are copied unchanged from that
working tree; `reference.yaml` records their SHA-256 digests and explicitly marks
the source state as uncommitted. All 704 local named resources have English
labels and definitions. The source registry explicitly
identifies the EKGF family and its 24 ontology members. DPROD and GraphArch are
not included.

The family CLI collects all sources into one KG and generates a standards-style
reference and PDF for 704 named resources, with cross-ontology links. Missing
English labels and definitions are reported as warnings in the reference and
retained as KG diagnostics. No prose definitions are invented, no imports are
fetched, and no SHACL validation or OWL reasoning is executed.

```sh
cargo run -- publication build-family \
  --source examples/ekg-ontologies/sources \
  --definitions examples/ekg-ontologies/project.ttl \
  --project urn:ekgf-family:project --target urn:ekgf-family:specification \
  --output-root output/pdf/ekgf-family
```

The integration test deletes its source copy before rendering from a restored
KG. Additional tests reject missing registered members, check source digests,
and distinguish documentation warnings from failed operations.

For live inputs, replace `--source examples/ekg-ontologies/sources` with
`--local-repository ../ekg-ontologies`, or
`--github EKGF/ekg-ontologies --git-ref main`. Both use the same specialized RDF
collectors and family pipeline. Remote sources use a reusable bare Git cache;
local sources include uncommitted edits. Neither mode copies files into this
fixture. The checked-in corpus exists for offline regression tests only.
