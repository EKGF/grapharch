# Authority-branded specification showcase

`project.ttl` supplies the document fragments, placements, publication metadata,
and reusable EKGF `ga:Authority`. The logo is an embedded PNG capture of the
existing symbol from `ekgf-website/public/ekgf-logo-symbol.svg` in the sibling EKGF
website repository (HEAD observed as `95cf8f3a93407d127fae83fb4d570df78989ad2b`).
No invented logo or W3C mark is substituted for EKGF.

```bash
cargo run --example publish_showcase -- --specification
cargo test --test showcases specification
```

The output is `output/pdf/grapharch-specification/specification/index.html`.
The integration tests exercise authority name/accent replacement, embedded PNG
validation, required metadata, output ownership, source deletion, nested numbering,
and preservation of the previous publication after invalid metadata changes.
The separate `w3c-rdf12-n-triples` fixture retains W3C authority instead.
