# OMG Commons Ontology Library

This showcase collects the published [OMG Commons Ontology Library](https://www.omg.org/spec/Commons)
from its authoritative RDF document URLs. It includes `AboutCommons.rdf` and the
22 ontologies explicitly imported by the Commons 1.3 convenience ontology,
covering names, identifiers, roles, organizations, collections, dates, quantities
and units, and other reusable patterns.

[`showcase.yaml`](showcase.yaml) declares every source and its RDF media type.
[`project.ttl`](project.ttl) contains only publication settings, family membership,
and the native ontology namespace/prefix declarations. No ontology files are
copied into this repository. We found no official public Commons Git repository;
using OMG's own published documents avoids treating a third-party mirror as the
authoritative source.

Every gallery build fetches the current content at the declared OMG URLs. The RDF
URL Collector retains the URL and SHA-256 content revision in the documentation
KG. This is an HTTP-source example, so there is no invented Git commit, tag, or
committer for these published inputs. External `owl:imports` are recorded, not
silently fetched: the declared list is the collection boundary. Network, syntax,
and publication failures fail the showcase and CI.

The output is an unofficial GraphArch documentation preview: a family overview,
one specification HTML page per ontology, and a combined PDF. Source definitions,
relationships and MIT notices are preserved. The preview does not claim OMG
approval and does not replace the official Commons specification.

The gallery runs it automatically. To verify this example independently against
the current OMG sources:

```sh
cargo test --test commons_showcase live_commons_showcase_from_restored_kg -- --ignored --nocapture
```

This acceptance run exports and restores the KG before rendering HTML and PDF in
`output/commons-review/specification/`. An offline HTTP regression separately
checks that rendering succeeds after the source directory and HTTP server are
removed. The generated KG can be inspected at
`output/commons-review/documentation.nq`.
