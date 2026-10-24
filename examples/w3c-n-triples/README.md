# RDF 1.1 N-Triples migration fixture

`source.html` is the unmodified published W3C Recommendation fetched from
<https://www.w3.org/TR/2014/REC-n-triples-20140225/> on 2026-10-24. Its digest is
pinned in `reference.yaml`. Preserve the document's copyright and notices.
Original publisher: World Wide Web Consortium (W3C), with the copyright holders
and document-use links recorded in the source header.

`w3c-logo.png` is the reference authority asset from
<https://www.w3.org/Icons/w3c_home.png>. `authority.ttl` contains its embedded PNG
representation so the renderer reads the logo from the KG without network I/O.
The PNG is retained for attribution and fixture verification.

```bash
cargo run --example publish_showcase -- --w3c
cargo test --test w3c_specification
```

The runner collects the document and authority metadata, assembles a validated
revision, resolves a specification target, and writes
`output/pdf/grapharch-w3c-n-triples/specification/index.html`. The exported
`documentation.nq` contains the content, native aliases, authority, provenance,
and plans needed for publication.

The collector exposes sections, defined terms, examples, and bibliography entries
as identified graph resources. Native HTML fragments preserve table/code structure
and public anchors; executable content is not admitted as publication machinery.
The integration test removes the input HTML and restores an exported KG before
rendering. It also verifies that unsupported executable attributes cannot replace
a valid collected revision.

This demonstrates semantic round-tripping, not exact W3C styling or a complete
ReSpec authoring-source importer. See the
[compatibility matrix](../../docs/other/architecture/respec-compatibility.md) for
remaining migration and visual acceptance work. Grapharch's output is a local
reproduction, not a newly issued W3C publication.
