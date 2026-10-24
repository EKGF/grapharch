---
title: Specification renderer
layout: default
parent: Usage
nav_order: 3
---

# Specification renderer

Use `ga:renderer "specification"` to publish a standards-style HTML document.
This is a dedicated renderer within the Grapharch CLI, distinct from the generic
HTML renderer's `ga:style "specification"` styling hint. It reads the same pinned
plans and validated graph revisions; there is no ReSpec or browser-side assembly.

The structure is informed by [OWL 2](https://www.w3.org/TR/owl2-overview/),
[Turtle](https://www.w3.org/TR/turtle/),
[DCAT 3](https://www.w3.org/TR/vocab-dcat-3/), and
[SHACL 1.2 Core](https://www.w3.org/TR/shacl12-core/): publication front matter,
abstract, document status, numbered sections, and a navigable table of contents.
Grapharch supplies its own CSS. The document's authority, status, copyright, and
license are graph assertions. W3C branding can be retained explicitly, or replaced
with EKGF or another authority, using the same renderer.

## Runnable showcase

```bash
cargo run --example publish_showcase -- --specification
cargo test --test showcases specification
cargo test --example publish_showcase
```

The output is `output/pdf/grapharch-specification/specification/index.html`, with
local `style.css` and an ownership manifest. The output root also contains the
exported KG. See `examples/specification/project.ttl` and the
[showcase catalog](../../examples/README.md).

## Publication metadata

A specification target requires these graph assertions:

| Predicate | Value |
| --- | --- |
| `dcterms:title` | Nonempty document title |
| `ga:publisher` | `ga:Authority` IRI with its own `dcterms:title` |
| `ga:publicationStatus` | Nonempty publisher-defined status, such as `Editor's Draft` |
| `ga:publicationDate` | Valid calendar date in `YYYY-MM-DD` form, typed `xsd:date` |
| `ga:thisVersion` | Absolute HTTP(S) IRI identifying this document version |
| `ga:latestVersion` | Absolute HTTP(S) IRI identifying the latest publication |

Optional target predicates are `ga:previousVersion`, `ga:editorDraft`, `ga:editor`,
`ga:author`, `ga:formerEditor`, `ga:license`, and `ga:copyright`. Editors are IRIs with `dcterms:title`, optional
`ga:affiliation`, and optional `ga:url` literals. A publisher may also have a
`ga:url` literal. Metadata links require absolute HTTP(S) URLs without credentials.
People are sorted by title and IRI unless explicit `ga:order` values are supplied.
If any person in a role has an order, every person in that role must have a distinct
nonnegative `xsd:integer` order. Published W3C collection records source order.
At most 64 people per role are accepted. Text fields and URLs have a 4096-byte runtime limit.

Publication metadata is validated by the renderer before artifact promotion.
The SHACL definitions document conditional specification requirements; the Rust
assembly validator does not yet execute all of these SHACL constraints. An
invalid publication does not replace the last successful generation.

## Reusable authorities

An authority node has `a ga:Authority` and `dcterms:title`. Optional fields are
`ga:shortName`, `ga:url`, `ga:accentColor`, and `ga:logo`. The accent must be a
six-digit `#RRGGBB` value. The logo is an embedded `data:image/png;base64,...`
literal, validated as PNG with limits of 256 KiB, 2048px per dimension, and 32 MiB
decoder allocation. Rendering does not fetch logos or reopen source assets.
An absent logo produces no image; it does not substitute another authority's mark.

The authority name/abbreviation appears in the publication status and vertical
banner. Its accent applies to headings and the banner. The optional logo appears
at the top right. The reading column is left aligned and limited to 50rem; the
fixed ToC uses separate number/label columns and a 27rem panel. Narrow screens
use the normal document order and an inline ToC. Wide native grammar tables use
a keyboard-accessible scroll region, preserving production names and separators.

The EKGF showcase embeds a PNG capture of the existing EKGF symbol from the
sibling website repository. It is a fixture asset, not an invented logo. Changing
only graph authority metadata is tested with another publisher name and accent.

## Published W3C document collection

```bash
cargo run --example publish_showcase -- --w3c
cargo run -- publication collect --store /tmp/grapharch-w3c --project urn:w3c:project --collector urn:w3c:collector --kind w3c-specification --source examples/w3c-n-triples/source.html --source-url https://www.w3.org/TR/2014/REC-n-triples-20140225/
```

This specialized collector accepts a local snapshot of supported published W3C
HTML, not unprocessed ReSpec authoring scripts. It emits publication definitions,
metadata, independently identified fragments, sections, defined terms, examples,
and bibliography entries. Original public anchors are retained. Examples without
native IDs receive content-derived IDs. Source DOM markup is retained separately
for provenance and fidelity comparisons.

The source URL is required for this collector, and is rejected for other collector
kinds. Input is limited to 2 MiB. Unsupported native elements, executable event
attributes, and unresolved local references fail. Inert RDF script examples
inside `pre` are emitted as escaped source text. Native `text/html` fragments
currently work only in the specification renderer; Markdown's raw-HTML restriction
remains unchanged. A target permits at most 20,000 native/generated anchors and
50,000 native links, within the shared body/entry budgets.

The pinned N-Triples fixture is tested after export/restore and source deletion.
[ReSpec compatibility](../other/architecture/respec-compatibility.md) records
remaining authoring-source migration and exact-output acceptance requirements.

## Sections and navigation

The profile must begin with exactly one placement having
`ga:sectionKind "abstract"`, followed by exactly one with `"status"`. Their titles
and bodies are authored content. These placements remain unnumbered. Subsequent
`"section"` placements receive numeric section numbers; `"appendix"` placements
receive lettered numbers. Glossary and reference placements remain unnumbered.
Main numbered sections cannot follow appendices.

Markdown headings are numbered relative to their enclosing component, using the
fragment's shallowest heading level as its baseline. Numbering continues across
fragments in the same component. The ToC uses the same numbering and includes
nested headings, while excluding abstract/status front matter. Stable anchors and section permalinks work without JavaScript.
Skipped heading levels and depths beyond HTML h6 fail explicitly. Shared body,
fragment, and entry budgets also apply to both streaming passes used to write
the ToC and document body.

The CSS provides a fixed desktop ToC, an inline ToC on narrow screens, visible
keyboard focus, a skip link, code and table styling, and print rules. It uses
system fonts and local assets only. This initial implementation does not include
an automatic bibliography engine, definition panels, requirement indexing,
change annotations, or complete W3C/OMG publication-rule validation.
