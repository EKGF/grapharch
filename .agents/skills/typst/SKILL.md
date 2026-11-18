---
name: typst
description: Author and debug Typst documents and Grapharch's native PDF renderer, including generated .typ source, publication styles, Markdown/HTML conversion, tables, code, references, and pagination. Use for Grapharch PDF layout or compilation defects and publication-template work. Does not cover unrelated website styling or PDF extraction.
---

# Grapharch Typst

Work from the repository's actual compiler and publication contract. Read its
`AGENTS.md` first. This skill does not authorize a dependency upgrade, new
publication capability, commit, or external publication.

## Establish the rendering context

1. Locate the repository root and read `Cargo.toml` and the resolved `typst`,
   `typst-library`, `typst-pdf`, and `typst-kit` entries in `Cargo.lock`.
   Run `typst --version` only if using the CLI: it can differ from the embedded
   compiler. At skill creation Grapharch embeds 0.13.1, while the local CLI is
   0.15.1. Recheck these facts rather than treating them as permanent pins.
2. Read [the pipeline map](references/grapharch.md) for the relevant source and
   tests. Inspect the current files, including uncommitted edits; the map is a
   navigation aid, not a substitute for the implementation.
3. Identify the target, pinned revision, profile, style, failing content, and
   generated output directory. Trace a defect to graph content, conversion,
   template layout, or compiler-world configuration before changing anything.

## Author against verified APIs

Consult [the authoring guide](references/authoring.md) for syntax, layout, and
version-specific reference lookup. Verify unfamiliar functions and parameters
against the matching Typst crate sources or tagged upstream sources before using
them. Current online documentation and upstream skills may describe newer APIs.

Use `set` rules for shared defaults, `show` rules for element transformations,
and existing `ga-*` helpers for reusable presentation. Keep content, publication
metadata, and style separate. Preserve source strings as data through the
existing quoting boundary; do not evaluate collected text as Typst code.

For Grapharch work, change maintained renderer/template/profile sources and
regenerate. Generated `.typ` files and `typst_debug/` copies are evidence or
scratch material, not the authoritative fix. A scratch document is useful for
isolating a language issue, but must not replace native pipeline validation.

## Prove the result

For a defect, add a focused failing regression first and record the failure.
Correct the smallest responsible layer and rerun the regression. Use
[the verification guide](references/verification.md) to choose the relevant
integration test and visual checks. A standalone CLI compilation does not prove
that Grapharch's embedded compiler, fonts, assets, or restored-KG flow works.

For layout changes, inspect rendered affected pages and boundary cases, as well
as generated source and extracted text. Report compiler version, checks run,
artifact paths, observed results, and remaining limitations. Compilation alone
does not establish readable pagination or correct links.

## Design sources

This is an independently written Grapharch-specific synthesis informed by:

- [apcamargo/typst-skills, typst-author](https://github.com/apcamargo/typst-skills/blob/main/typst-author/SKILL.md): documentation-led syntax verification and consistent presentation.
- [Ulrich Atz's statzhero/typst-skill](https://github.com/statzhero/typst-skill/blob/main/SKILL.md), published under CC-BY-4.0: task-focused reference routing and separation of shared styling from content.

Reviewed 2026-11-09. No upstream manual, template, or code is vendored here;
the local workflow and references below are based on Grapharch's working tree.
