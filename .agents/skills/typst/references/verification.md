# Verification in Grapharch

## Choose the relevant acceptance path

Read the selected test before running it; test names and fixture coverage can
change. Add a regression to the closest existing suite for a demonstrated defect.

| Behavior | Starting integration suite |
|---|---|
| Profile skins, long ribbon, preface, paired PDF | `tests/pdf_presentation.rs` |
| Community and OMG editions from a restored KG | `tests/dprod_editions.rs` |
| Release metadata and authority requirements | `tests/standards_profiles.rs` |
| Legal front matter | `tests/publication_preamble.rs` |
| Anchors and footnotes | `tests/explicit_anchors.rs`, `tests/footnotes.rs` |
| Retained images without original files | `tests/collected_images.rs` |
| Code and Mermaid diagrams | `tests/code_examples.rs`, `tests/mermaid_diagrams.rs` |

For example, from the repository root:

```sh
cargo test --test pdf_presentation
cargo test --test dprod_editions
```

Run only suites relevant to the change, then the repository-required checks.
For shared pipeline changes, follow `AGENTS.md`'s broader testing requirement.
For a bug, demonstrate the new test fails for the intended reason before fixing
production code. Preserve source-deletion/export/restore assertions when extending
a test; source-string assertions or `%PDF-` checks alone cannot prove correctness.

## Generate and inspect a real publication

The existing DPROD example exercises the native publisher:

```sh
cargo run --example publish_showcase -- --dprod output/typst-review
```

Inspect the example's current argument parser and declared target destinations
before assuming where its PDFs land. Custom edition tests may create profiles
that the stock example does not declare. Use the fixture or real target that
actually exercises the changed behavior. Generated material belongs in ignored
`output/` directories or a temporary directory, not in the skill or source tree.

Inspect generated `main.typ` for text quoting, labels, metadata, helper selection
and ordering. Then inspect the PDF with tools available in the environment:

```sh
pdfinfo output/typst-review/pdf/document.pdf
pdftotext -layout output/typst-review/pdf/document.pdf output/typst-review/pdf/document.txt
pdftoppm -f 1 -l 3 -scale-to 1600 -png output/typst-review/pdf/document.pdf output/typst-review/pdf/page
```

Substitute the observed output path and affected page range. Open the rendered
images with an available image viewer. Text extraction can find missing text,
but cannot prove that content fits. Raster images cannot prove link destinations;
inspect PDF annotations with an available PDF library when links change.

Review the cover, contents/front-matter transition, affected chapter opening,
continuation page, and appendix as applicable. Check:

- No overlap, clipping, unintended blank pages, missing glyphs or stranded titles.
- Long IRIs, identifiers, code, captions and repeated table headers remain readable.
- Running heads, page totals, chapter numbering, references and outline entries
  agree with the actual publication structure.
- Styles keep their intended geometry and typography. Unselected content does
  not leak into an edition; selecting a skin does not invent approval metadata.
- Native compilation produces no warnings and works with embedded fonts and
  retained assets after original-source removal.

## Skill acceptance scenarios

Use these to review behavior when updating this skill. They are manual scenarios,
not claims that an autonomous-agent benchmark has passed:

1. “The first overview chapter heading is too close to its body.” Trace the
   current generated container and theme; add a layout regression and inspect the
   affected PDF pages. Do not patch a generated `typst_debug/` copy.
2. “Use this 0.15 documentation example in our PDF.” Check the resolved embedded
   compiler and its API before adopting it. A newer local CLI is insufficient
   evidence; a compiler upgrade requires its own authorized scope.
3. “The OMG PDF loses a diagram after sources are removed.” Inspect retained KG
   assets, conversion and restored-dataset tests. Do not repair it with a source
   reread or network download at rendering time.
4. “Fix the website's mobile navigation.” This skill should not route ordinary
   website styling through the Typst renderer.

Report validation honestly: distinguish skill-structure validation, manual
scenario review, compiler execution, and visual review. Do not claim PDF rendering
or behavioral benchmarking when only this instruction package was changed.
