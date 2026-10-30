---
title: Specification renderer
layout: default
parent: Usage
nav_order: 3
grapharch:
  subject: urn:grapharch:docs:topic:usage:specification-renderer
permalink: /usage/specification-renderer/
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
[showcase catalog](examples.md).

## Publication metadata

A specification target requires these graph assertions:

| Predicate | Value |
| --- | --- |
| `dcterms:title` | Nonempty document title |
| `ga:publisher` | Authority role IRI, typed `artgov:Authority` or its equivalent `ga:Authority`, with an English `rdfs:label` |
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

An authority node denotes a governance role, typed `artgov:Authority` or its
equivalent `ga:Authority`. It requires an English `rdfs:label` and an abbreviation
using `artgov:authorityAcronym` or the equivalent `ga:shortName`. Its player is a
distinct party linked through Commons `isPlayedBy`, when known. Reuse the UUIDs
in the EKGF authority registry; hosts and URLs do not determine identity.
Optional presentation fields are `ga:url`, `ga:accentColor`, and `ga:logo`. The accent must be a
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
cargo run --example publish_showcase -- --w3c --keep-going
cargo run -- publication collect --store /tmp/grapharch-w3c --project urn:w3c:project --collector urn:w3c:collector --kind w3c-specification --source examples/w3c-rdf12-n-triples/source.html --source-url https://www.w3.org/TR/2026/WD-rdf12-n-triples-20261023/
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

On desktop screens, drag the right edge of the table of contents to resize it.
The focused separator supports Left/Right arrow keys (Shift for larger steps),
Home/End for its minimum/maximum width, and double-click to reset. The browser
remembers the width across specification pages. On smaller screens the contents
remain in the document flow; printed output has no resize control.

## Large families: one specification page per ontology

Select `ga:renderer "specification-pages"` on a family publication target to
publish a collection of smaller standards-style HTML pages. The family overview
stays at `index.html`; each remaining logical component gets its own page. The
ontology-family documentor already supplies one component per ontology and one
per explicitly declared module, so this produces ontology and module pages
without guessing boundaries from filenames or splitting a generated HTML book.

```turtle
<urn:example:specification>
    ga:renderer "specification-pages" ;
    ga:destination "specification" .
```

Use this setting **instead of** the target's existing `ga:renderer` value;
conflicting renderer values are errors. All other scope, profile, authority,
status, date, and source hints keep their existing meaning. Run the normal
`publication build-family` command with that target.

Each ontology page has the familiar specification header, local numbered
sections, and a local contents list. Family navigation retains the module tree.
Routes use stable placement identities, and links between documented terms point
to the page and anchor that own the referenced content. Styles, images, page
routes, and the shared `search-index.json` use relative URLs and work beneath a
GitHub Pages project path. Rendering requires only the resolved plan and its
pinned KG, including when the original source files have been deleted.

This renderer publishes HTML **without compiling a PDF**. Use a separate PDF or
single-document specification target when a book is wanted. The existing
`specification` renderer still produces a single standards page and its PDF.
A companion marketing or guide website remains a separate `website` target.

## Standards publication requirements

A profile can require semantic sections independently of their titles and
numbering. The planner checks the sections selected for that target; content
elsewhere in the KG, in a different profile, or excluded by scope does not count.

```turtle
ex:release-profile a ga:Profile ;
    ga:requiredSectionRole "abstract", "scope", "conformance", "references",
        "terms", "symbols", "namespaces", "acknowledgements", "model", "examples" ;
    ga:requiredMetadata dct:title, ga:publisher, ga:preamble,
        ga:publicationDate, ga:releaseVersion, ga:documentNumber ;
    ga:allowedPublicationStatus "Formal Specification" .

ex:scope-placement ga:sectionRole "scope" .
```

Section roles are explicit hints on placements, distinct from `ga:sectionKind`,
which controls structural presentation. Required metadata is identified by its
RDF predicate, including predicates from other vocabularies. Missing values and
empty literals fail planning. An optional status allowlist prevents a preview
from satisfying a release profile. Failures become KG diagnostics against the
observed revision; no incomplete publication plan is promoted.

Use separate draft and release profiles. Drafts can carry the full technical
content while an authority-assigned document number or release notice is still
unknown. Require those values in the release profile instead of inventing them.
When supplied, `ga:releaseVersion` and `ga:documentNumber` appear in the
specification header and PDF cover metadata.

## Legal preamble

Link a specification or PDF target to a retained fragment with `ga:preamble`.
The fragment's title supplies the disclosure label; its complete legal wording
remains in the KG alongside its source provenance.

```turtle
ex:specification ga:preamble ex:legal-notices .
ex:legal-notices a ga:Fragment ;
    dct:title "Legal notices" ;
    ga:subject ex:publication ;
    ga:contribution ex:authored ;
    ga:format "text/markdown" ;
    ga:body "## Copyright\n\nCopyright © 2026 Example Authority.\n\n## Terms\n\nThe full source-owned legal terms go here." .
```

Specification HTML presents a keyboard-accessible disclosure, closed initially.
The PDF includes the entire preamble on dedicated front-matter pages after the
cover and before the contents. Its typography and accent follow the publication
theme. It does not become a numbered technical chapter. HTML and Markdown
fragments are supported; unsupported formats and missing legal fragments fail
the publication rather than silently omitting the notices.

For a ReSpec migration, `HtmlPreambleFile` collects the unique
`details.preamble-section` block without running scripts or template code.
The DPROD live showcase reads that block from `respec/template.html` in the same
cached `develop` commit as its ontology and shapes. Its legal wording is not
copied into GraphArch's repository.

## Contents navigation and search

Both specification renderers provide collapsible contents branches, **Collapse
all** and **Expand all** controls, and a **Search & filter** dialog. Open search
with the button or **Ctrl K** / **Command K**; close it with **Escape**. Search
matches heading titles and, for paginated publications, page subject identifiers
and ontology/module context. Filters select ontologies, modules, or sections and
terms. Results are limited to the first 50 matches; refine the query to narrow
them down. This is a navigation index, rather than full-text body search.

The paginated renderer shares one search index across the family and loads it
when search is first opened. Its **Browse this family** disclosure loads a shared
navigation index only when expanded, instead of duplicating the full family tree
in every HTML page. Publications are bounded to 8,192 pages; navigation and
search index content is derived from the pinned plan. A failed index load is displayed as an error.
The single-page renderer indexes its document headings, including headings
omitted from the visible contents by `ga:tocDepth`. Links remain usable with
JavaScript disabled; collapse and search controls require JavaScript.

For a library of module books, each with its own PDF and chapter navigation, use
[the bookshelf renderer](bookshelves.md) instead of a family-wide specification.
