---
title: Examples and showcases
parent: Usage
nav_order: 5
permalink: /usage/examples/
grapharch:
  subject: urn:grapharch:docs:topic:examples
---

# Executable showcases

Browse the [showcase gallery](../showcases.md) for live HTML previews, PDFs,
and downloadable documentation graphs.

These fixtures are both runnable demonstrations and integration tests. The runner
and `tests/showcases.rs` use the same pipeline in `support/mod.rs`; tests do not
maintain a second implementation of collection or publication.

| Showcase | Native inputs | Processing | Outputs |
| --- | --- | --- | --- |
| `showcase/` | Authored RDF containing Markdown fragment bodies and publication definitions | RDF collection, assembly, three target plans | Specification HTML, reader HTML, review PDF |
| `w3c-rdf12-n-triples/` | Pinned RDF 1.2 N-Triples Working Draft | Published-document collector, assembly, specification target | HTML and PDF, native anchors and queryable constructs, plus a diagnostic addendum for an upstream broken reference |
| `specification/` | Authored RDF containing publication metadata and Markdown fragments | RDF collection, assembly, specification target plan | Standards-style HTML with nested numbered ToC, publisher front matter, abstract, status, and appendix |
| `dprod/` | Pinned DPROD ontology, SHACL shapes, and dataset-schema Markdown from the DPROD repository | RDF and Markdown collection, 53 annotated-term fragments, assembly, three target plans | Specification HTML, companion website, review PDF, exported KG |
| `ekg-ontologies/` | Latest default branch of a configured remote ontology repository; separate offline acceptance corpus | Parallel RDF collection and source-annotation documentation, explicit family membership, cross-ontology links | Family specification, PDF, exported KG with annotation-gap diagnostics |
| `mixed-sources/` | Separate Markdown, SPARQL, OWL, and publication-definition files | Four specialized collectors scheduled concurrently, OWL class documentor, assembly, two target plans | Reader HTML, review PDF |

From the repository root:

```bash
cargo run --example publish_showcase
cargo run --example publish_showcase -- --mixed
cargo run --example publish_showcase -- --specification
cargo run --example publish_showcase -- --dprod
cargo run --example publish_showcase -- --w3c --keep-going
cargo test --test showcases
```

Pass an optional positional output root to any invocation. Defaults are
`output/pdf/grapharch-showcase`, `output/pdf/grapharch-mixed-sources`, `output/pdf/grapharch-specification`, and
`output/pdf/grapharch-dprod`, and `output/pdf/grapharch-w3c-rdf12-n-triples`.
The RDF 1.2 keep-going run deliberately exits nonzero after producing diagnostic
artifacts because its upstream source has an unresolved link.
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

The [ontology-family guide](ontology-families.md) shows how to run the EKGF family
from a remote repository, a local checkout, or the separate offline acceptance corpus.
The published gallery fetches the latest default branch declared in
`examples/ekg-ontologies/showcase.yaml`; its downloadable KG records the concrete
commit observed by each build.

## Adding a showcase

Keep native input files readable and small. Declare what the example demonstrates
and its limits in this catalog. Put shared pipeline code in `support/` and add a
behavioral integration test before implementing the new path. Assert observable
content, provenance, ordering, failure behavior, or graph relationships; avoid
byte-for-byte PDF snapshots and copying renderer internals into tests.

The real N-Triples fixture proves semantic round-tripping and native anchor
preservation; it does not yet establish exact W3C visual parity. ReSpec is publicly
named as a reference in the [compatibility map](../other/architecture/respec-compatibility.md).

The DPROD fixture documents four OWL classes, thirteen object properties, one datatype
property, one SKOS concept scheme, six SHACL node shapes, and twenty-eight property
shapes. Its test deletes the sources and renders from a restored graph. Source
files are unchanged copies pinned to a DPROD commit with checked digests. This is
a [three-publication family](publication-families.md) and derivative reference preview, not a full migration of the DPROD specification
or an official OMG publication.

Standalone SKOS vocabularies and use-case trees still need their own fixtures.

### Live DPROD gallery

The gallery reads `EKGF/dprod` directly from its remote `develop` branch.
`examples/dprod/live.yaml` specifies only the repository, branch and location of
its source-owned publication settings. Each build resolves one Git commit and
collects its ontology, shapes and Markdown at that revision. A reusable local
Git cache avoids cloning the complete repository again. Missing source files,
invalid hints and unavailable sources stop the build; copied fixtures are never
used as a replacement.

Repository links resolve to published companion pages where available, or to
verified files at the same commit on GitHub. Images are collected into the
publication KG, checked for supported content and size, and restored from that
KG when producing HTML and PDF. Renderers do not fetch images or reopen Git.

The older pinned corpus in `examples/dprod/` remains an offline regression
fixture for `publish_showcase --dprod`; the live gallery uses the remote source.
Run its network acceptance explicitly with
`cargo test --test live_dprod -- --ignored`.
