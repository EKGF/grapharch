# Executable showcases

These fixtures are both runnable demonstrations and integration tests. The runner
and `tests/showcases.rs` use the same pipeline in `support/mod.rs`; tests do not
maintain a second implementation of collection or publication.

| Showcase | Native inputs | Processing | Outputs |
| --- | --- | --- | --- |
| `showcase/` | Authored RDF containing Markdown fragment bodies and publication definitions | RDF collection, assembly, three target plans | Specification HTML, reader HTML, review PDF |
| `w3c-n-triples/` | Pinned published W3C RDF 1.1 N-Triples HTML and authority asset RDF | Published-document collector, assembly, specification target | W3C-branded HTML with native anchors and queryable sections, terms, examples, and bibliography |
| `specification/` | Authored RDF containing publication metadata and Markdown fragments | RDF collection, assembly, specification target plan | Standards-style HTML with nested numbered ToC, publisher front matter, abstract, status, and appendix |
| `mixed-sources/` | Separate Markdown, SPARQL, OWL, and publication-definition files | Four specialized collectors scheduled concurrently, OWL class documentor, assembly, two target plans | Reader HTML, review PDF |

From the repository root:

```bash
cargo run --example publish_showcase
cargo run --example publish_showcase -- --mixed
cargo run --example publish_showcase -- --specification
cargo run --example publish_showcase -- --w3c
cargo test --test showcases
```

Pass an optional positional output root to any invocation. Defaults are
`output/pdf/grapharch-showcase`, `output/pdf/grapharch-mixed-sources`, `output/pdf/grapharch-specification`, and
`output/pdf/grapharch-w3c-n-triples`.
Each output root includes `documentation.nq`, an export containing the retained
contributions, validated revision, pinned plans, and publication-job records.

The authored example demonstrates shared content with different target ordering,
styles, table rendering, code, glossary, references, and appendices. Its test
exports and restores the dataset into a fresh store before rendering, after
removing the original input. It checks shared revision provenance, common content,
different section ordering, and PDF production.

The mixed example contains no authored `ga:body` triples. Markdown frontmatter and
a SPARQL comment pragma identify documentable subjects. An OWL `rdfs:comment`
becomes a documentation fragment through `OwlClassDocumentor`. The query is
published as source text, without execution. Its test removes every input before
rendering and checks that both outputs contain prose from all three source types.

The mixed runner explicitly binds components to the stable fragment IDs exposed
by the collectors and documentor. File enumeration and bindings are example
wiring in Rust; `ga:source` declarations do not yet drive automatic discovery.
Only the OWL class description is placed in these outputs; generated assertion
fragments remain in the graph. This is not full ontology documentation coverage
or a DPROD acceptance fixture.

The specification example tests publication metadata, nested numbering, local CSS,
escaped content, and output from a KG after source deletion. Invalid status/date
metadata and unsafe publication links must preserve the previous output. A runner
regression test verifies that its default output root differs from the original
authored showcase.

## Adding a showcase

Keep native input files readable and small. Declare what the example demonstrates
and its limits in this catalog. Put shared pipeline code in `support/` and add a
behavioral integration test before implementing the new path. Assert observable
content, provenance, ordering, failure behavior, or graph relationships; avoid
byte-for-byte PDF snapshots and copying renderer internals into tests.

The real N-Triples fixture proves semantic round-tripping and native anchor
preservation; it does not yet establish exact W3C visual parity. ReSpec is publicly
named as a reference in the [compatibility map](../docs/other/architecture/respec-compatibility.md).

The next distinct scenarios are SHACL constraints, SKOS vocabularies, use-case
trees, and a real DPROD subset. They need their own fixtures and acceptance tests;
none is represented by the generic prose in the authored example.
