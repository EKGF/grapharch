# RDF 1.2 N-Triples reference showcase

This is the primary W3C specification reference, using the unchanged published
23 October 2026 Working Draft. `reference.yaml` pins source URLs and digests.
The older RDF 1.1 fixture remains in `../w3c-n-triples`.

Run `cargo run --example publish_showcase -- --w3c --keep-going`.
The command deliberately exits with an error after producing the preview: the
upstream source contains an unresolved `#must-signal-errors` reference, already
marked as broken by ReSpec. Grapharch records this as a KG diagnostic and adds an
errors addendum to both HTML and PDF. Without `--keep-going`, rendering fails.

Artifacts are written under `output/pdf/grapharch-w3c-rdf12-n-triples`.
The renderer consumes the collected KG and pinned publication plan. It does not
execute ReSpec or reread the original source during rendering. Modern metadata,
contributors, front-matter contents, native anchors, examples, grammar tables,
and bibliography are retained. Exact visual parity remains a separate acceptance
goal; this showcase does not claim pixel-identical W3C output.
