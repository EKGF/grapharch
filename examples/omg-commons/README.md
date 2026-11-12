# OMG Commons Ontology Library

This showcase collects the published [OMG Commons Ontology Library](https://www.omg.org/spec/Commons)
from its authoritative RDF document URLs. It includes `AboutCommons.rdf` and the
22 ontologies explicitly imported by the Commons 1.3 convenience ontology,
plus the separately published mapping to OWL-Time,
covering names, identifiers, roles, organizations, collections, dates, quantities
and units, and other reusable patterns.

[`showcase.ttl`](showcase.ttl) declares every source as a `ga:rdfDocument` with its `ga:format` media type,
using OMG's versioned 1.3 URLs (each module's `owl:versionIRI` date, for
example `https://www.omg.org/spec/Commons/20250801/AboutCommons.rdf`), so the
edition does not change when OMG publishes a later release. On 2026-11-12 the
versioned documents had the same content as the unversioned URLs, apart from the
line endings of `Classifiers.rdf` and `MappingDatesAndTimesToOWLTime.rdf`.
[`project.ttl`](project.ttl) contains the publication settings, collection
membership, the native ontology namespace/prefix declarations, and the OMG
edition: house-style theme, cover, notices, preface, Annex A and release
metadata. The collection annotates
`ga:collectionTerm "library"`, so its overview, navigation, search and PDF use
“library” consistently while retaining the existing collection identity and membership. No ontology files are
copied into this repository. We found no official public Commons Git repository;
using OMG's own published documents avoids treating a third-party mirror as the
authoritative source.

Every gallery build fetches the content at the declared OMG URLs. The RDF
URL Collector retains the URL and SHA-256 content revision in the documentation
KG. This is an HTTP-source example, so there is no invented Git commit, tag, or
committer for these published inputs. External `owl:imports` are recorded, not
silently fetched: the declared list is the collection boundary. Network, syntax,
and publication failures fail the showcase and CI.

## The OMG edition

The PDF follows the layout of the formal Commons 1.3 specification
(formal/26-06-05). OMG's typography, table colours, page numbering, contents and
annex conventions are values of the consumer theme `omg:theme` in `project.ttl`;
the GraphArch engine contains no OMG-specific code. The cover shows the OMG logo,
the date, the title, "Version 1.3", the document number and the normative
reference. Its Edition row states that this is a GraphArch rendering of the
Commons 1.3 formal specification. The HTML reference and the PDF share the
profile; the PDF omits the library overview, the documentation coverage and
quality appendices, the provenance appendix, and the diagram notes.

- The logo is OMG's own
  <https://www.omg.org/images/logos/OMG-logo-web-sm.svg>, embedded unmodified
  as an SVG data URI, SHA-256
  `16869bd64078bd6068bb5fa3e029ed7faff201a91c3c9a79831708dca931dae0`
  (retrieved 2026-11-12).
- The copyright holders, the terms and conditions, the issue reporting
  procedure, the preface and Annex A are transcribed from the formal/26-06-05
  PDF, <https://www.omg.org/spec/Commons/1.3/PDF>. The boilerplate is the same
  as the OMG specification template, mdsa-omg-core at 7e9dbce,
  `Specification_Template.tex`.
- The publication date, 2026-03-24, is the creation date of that PDF.

Clauses 1 to 7 and Annex B of the formal specification are not yet authored
here, so the ontology chapters are numbered under clause 1 rather than 8.

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
