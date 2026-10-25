---
title: Specification, companion site, and PDF
parent: Usage
nav_order: 6
permalink: /usage/publication-families/
grapharch:
  subject: urn:grapharch:docs:topic:publication-families
---

# One knowledge graph, distinct publications

An ontology project can publish a specification, a companion website, and a PDF.
They are separate publications with separate target identities. They share
identified resources and selected fragments, rather than being three skins over
an identical document.

| Publication | Reader purpose | Typical content | Renderer |
| --- | --- | --- | --- |
| Specification | Find the authoritative definition | Terms, constraints, conformance, references, publication status | `specification` |
| Companion website | Understand the ideas and learn to use them | Overview, stories, guides, tutorials, showcases | `website` |
| PDF edition | Review or approve a document | Explicit document order, contents, references, glossary, appendices | `typst` |

Authority comes from the identified publisher and publication process. A
standards-style layout alone does not confer approval. Draft and final editions
must declare their actual status.

## Model each publication explicitly

A `ga:Project` can have several `ga:Target` resources. Each target binds a scope,
profile, renderer, and destination. A scope chooses which documented subjects
belong in that publication. A profile arranges their components using placements.

A specification and PDF may share a scope while using independent profiles. A
companion site usually has a different scope and profile: its guides should not
silently become normative specification sections. A shared explanation can be
reused deliberately in more than one profile.

For example, these are distinct target definitions, not renderer-wide global
settings:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix ex: <urn:example:> .

ex:project a ga:Project ; ga:target ex:spec, ex:site, ex:pdf .
ex:spec a ga:Target ;
    ga:scope ex:reference-content ; ga:profile ex:spec-order ;
    ga:renderer "specification" ; ga:destination "spec/develop" .
ex:site a ga:Target ;
    ga:scope ex:guides-and-stories ; ga:profile ex:site-navigation ;
    ga:renderer "website" ; ga:destination "companion" .
ex:pdf a ga:Target ;
    ga:scope ex:reference-content ; ga:profile ex:approval-document ;
    ga:renderer "typst" ; ga:destination "documents/develop" .
```

This snippet shows the target relationships; a complete project also declares
scope members, profiles, placements, fragments, and specification metadata.
Output directories must be separate and non-overlapping. Deployment can mount
the companion output at `/dprod/`, the specification at `/dprod/spec/develop/`,
and the document at an appropriate download URL. Those public URLs and the local
artifact directories are different concerns. Combined deployment and cross-target
navigation are not automatically configured by this snippet.

## Pin the family to one revision

Collect and document the sources, assemble one accepted revision, then resolve
each target against that revision. Each target gets its own pinned plan and
publication-job record. Readers can identify exactly which source revision each
publication represents, even when its selection and arrangement differ.

The [DPROD showcase](examples.md) exercises this flow. It publishes:

- A specification containing generated ontology and SHACL reference fragments.
- A multipage companion website containing an introduction, a story, and a guide.
- An independently planned review PDF, with the worked example arranged as an appendix.

The companion prose is excluded from the specification and document profiles.
The test removes the input files, restores the exported KG, and renders all three
targets. It checks shared revision provenance and separate profiles.

The specification renderer currently also emits a paired PDF for its own download
button. That is a rendition of the specification target; it is distinct from the
independently planned review document. Automatically linking that button to an
arbitrary alternative PDF target is not implemented.

## CLI and implementation boundaries

All these renderers are part of the same Rust library and CLI. The persistent
`publication assemble`, `publication plan`, and `publication render` operations
can work with explicit targets. The DPROD showcase runner collects and documents
once, then resolves and renders all three targets:

```sh
cargo run --example publish_showcase -- --dprod
```

Its output root is `output/pdf/grapharch-dprod`, with `specification/`, `website/`,
and `pdf/` directories. The `publication build-site` convenience command builds a
Markdown website target; it is not yet a general project configuration runner
for arbitrary mixed collector and documentor pipelines.
