---
title: ReSpec compatibility
layout: default
parent: Architecture
nav_order: 2
---

# ReSpec compatibility and migration

[ReSpec](https://github.com/speced/respec) is an explicit reference for Grapharch's
specification features and migration behavior. Its
[documentation](https://respec.org/docs/), source modules, tests, and published
outputs provide acceptance examples. Grapharch does not require ReSpec at runtime.

The source review on 2026-10-24 used upstream version **37.4.1**, revision
[`4b35fa2b43891577773e50968e700e6145d12baf`](https://github.com/speced/respec/tree/4b35fa2b43891577773e50968e700e6145d12baf).
The [upstream license notice](../references/respec-LICENSE.txt) is retained with
this reference inventory. No upstream JavaScript is bundled or executed by the
Grapharch renderer. Retain source attribution and applicable notices when adding
reference fixtures or incorporating upstream material.

## Publication authority and rendering engine are separate choices

A target selects a renderer and a profile. `ga:publisher` selects a reusable
`ga:Authority`: name, short name, logo, homepage, and accent color. W3C, EKGF, OMG,
EDMA, and other authorities use the same mechanism. An authority change must not
require a renderer fork or rewriting document bodies. Actual status and process
text remain explicit graph content, independent of visual branding.

A publisher-neutral specification style and faithful W3C output are both required
use cases. W3C output retains the selected W3C authority; it is not forcibly
rebranded. Existing Grapharch CSS is an initial interpretation, **not yet an
exact W3C output profile**.

## Feature map

Statuses describe tested Grapharch behavior, not a claim of ReSpec equivalence.
All upstream source links below are pinned to the reviewed revision.

| ReSpec feature/reference | Grapharch equivalent | Current evidence and gaps |
| --- | --- | --- |
| [`w3c/headers.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/w3c/headers.js): title, subtitle, versions, status, date, editors, logos, other links | Target metadata plus `ga:Authority` | Authored and published-document fixtures render these fields. Branding replacement is tested. Editor/author/former-editor roles and explicit person ordering are supported; full header-link coverage, labels, and all status combinations remain incomplete. |
| [`core/structure.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/core/structure.js), `tests/spec/core/structure-spec.js` | Profile placements, numbered outline, ToC, native aliases | Nested headings, appendices, abstract/status front matter, and native anchor resolution are tested. `maxTocLevel`, `noTOC`, unusual introductory sections, and complete numbering parity remain open. |
| [`core/dfn.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/core/dfn.js), `core/link-to-dfn.js`, `core/dfn-panel.js` | Identified terms and graph references | Published `dfn` resources and their native markup are collected. Alias/plural resolution, automatic linking, cross-spec indexes, and definition panels are not implemented. |
| [`core/biblio.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/core/biblio.js), `core/render-biblio.js` | Bibliographic resources and reference components | Published reference lists are preserved, including native anchors. ReSpec citation syntax, `localBiblio`, bibliography lookup, and automatic normative/informative classification are not implemented. |
| [`core/examples.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/core/examples.js), `core/issues-notes.js`, `core/figures.js` | Example/figure/annotation resources and fragments | Published examples are queryable and displayed; RDF script examples become escaped source text. Complete labels, figure assets, issue/note boxes, captions, and indexes remain open. |
| [`core/data-include.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/core/data-include.js), `core/markdown.js` | Specialized collectors, source contributions, normalized fragments | Independent Markdown files work. ReSpec include/transformation syntax and dependency reconciliation are not implemented. |
| [`w3c/conformance.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/w3c/conformance.js), `w3c/group.js`, `w3c/linter-rules/` | Authority/process metadata, constraints, conformance components | Authored status and requirement text can be published. W3C policy templates, group lookup, legal boilerplate generation, and publication-rule parity are not implemented. |
| [`core/webidl.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/core/webidl.js), `core/cddl.js`, syntax highlighting | Specialized construct collectors/documentors and code fragments | Plain source blocks are supported. WebIDL/CDDL semantics and HTML syntax-highlighting parity are not implemented. |
| [`core/exporter.js`](https://github.com/speced/respec/blob/4b35fa2b43891577773e50968e700e6145d12baf/src/core/exporter.js), `tools/respec2html.js` | Static artifacts from pinned graph plans | HTML/CSS and embedded Typst/PDF publishing work. Native published HTML fragments currently target the specification renderer only; a shared native-HTML-to-Typst mapping remains open. |
| `respecConfig`, `preProcess`, `postProcess`, `data-transform` | Declarative migration adapters and explicit conversion plugins | Published HTML can be collected without executing JavaScript. Import of authoring-time JavaScript configuration or hooks is not implemented. Unsupported executable hooks must be reported, not silently dropped or evaluated. |

## Real-standard acceptance fixtures

The first fixture is the published
[RDF 1.1 N-Triples Recommendation of 25 February 2014](https://www.w3.org/TR/2014/REC-n-triples-20140225/).
`examples/w3c-n-triples/source.html` is a pinned reference input. Its digest,
source URL, and attribution are recorded beside it. Integration tests run offline
against that fixture. They collect content into the shared KG, assemble a target,
export and restore the graph, delete the source, and render from the restored KG.
They check publication metadata, section/term/example/reference resources, native
anchors, and representative grammar, tables, and source blocks.

The specialized published-document collector is an initial migration path and a
way to populate behavioral tests. It does not substitute for importing a ReSpec
authoring repository. Source markup is retained as provenance; rendered sections
are independently identified fragments, not an opaque copy of the original page.

## Exact-output acceptance contract

Exact W3C compatibility must be established against a named, versioned output
profile and pinned inputs. A passing content test is insufficient. Acceptance
requires:

1. Equivalent content, section numbering, document order, normative/informative
   distinctions, definitions, references, examples, tables, and code.
2. Preserved public anchors and working internal/external links, with deterministic
   identity and explicit reports for unresolved references.
3. Equivalent header metadata, authority assets, status/process text, and notices.
4. Pinned CSS/fonts/assets and browser/viewport settings, with reviewed visual
   comparisons for desktop, narrow-screen, and print layouts.
5. Equivalent supported interactions and accessibility behavior, with every
   deliberate difference documented.
6. Reproduction after source removal and dataset restore, with no implicit fetches
   of mutable upstream content during rendering.

Neither byte-identical HTML nor pixel-identical W3C output is claimed today.
Golden visual comparisons, complete style/asset capture, and authoring-config
migration are remaining acceptance work. The DPROD repository also references an
EKGF ReSpec build (fork HEAD observed as
`b35286e4e5bd3b0499dda41cb83ba751f0753e7c` on 2026-10-24);
its fork-specific behavior needs a separate audit before a
DPROD replacement can be called compatible.
