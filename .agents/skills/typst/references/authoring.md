# Authoring and reference lookup

## Version-aware documentation

Start with the resolved compiler version, not the newest online manual.
If the Cargo registry cache is present, find the corresponding sources:

```sh
rg --files ~/.cargo/registry/src | rg '/typst-(library|syntax|layout)-0\.13\.1/'
```

Replace the version with the one actually resolved. Function declarations,
parameter docs, and examples in these sources can settle availability questions.
For missing local material, inspect the matching tag in
`https://github.com/typst/typst/tree/v0.13.1` (substitute the actual version).
Do not treat a changelog URL as a frozen version of the entire online manual.

The following official pages are useful conceptual references, but describe the
current release; verify APIs against the resolved source before adopting them:

| Task | Reference |
|---|---|
| Markup, code, math modes and escaping | https://typst.app/docs/reference/language/syntax/ |
| Set/show rules and selectors | https://typst.app/docs/reference/language/styling/ |
| Context, counters, running heads | https://typst.app/docs/reference/language/context/ |
| Margins, page numbering, headers and footers | https://typst.app/docs/guides/page-setup/ |
| Repeated headers, cells, spans | https://typst.app/docs/guides/tables/ |
| Figures and captions | https://typst.app/docs/reference/model/figure/ |
| Bibliographies and citations | https://typst.app/docs/reference/model/bibliography/ |
| Equations and math syntax | https://typst.app/docs/reference/math/ |
| Semantic structure and export accessibility | https://typst.app/docs/guides/accessibility/ |

Do not introduce Context7, Quarto, Touying, or CeTZ as required dependencies.
They are not prerequisites of Grapharch's native PDF flow. If the task requests
such an integration, first establish how it can run in the embedded World.

## Syntax and data boundaries

- Markup is the document default; `#` enters code. Content uses `[...]`, code
  blocks use `{...}`, arrays use parentheses, and dictionaries use named entries.
  A one-element array needs a trailing comma; an empty dictionary is `(:)`.
- A string is not markup. Emit source-owned prose through `#text(quoted-value)`
  or the existing conversion helpers; quotation marks, backslashes, control
  characters and literal `#`, `[]`, `$`, `@`, and angle brackets need coverage.
  Use `markup::quote` rather than building another partial escaping scheme.
- Source snippets belong in the existing raw/highlighted-code pipeline. Do not
  use `eval` to render prose or code examples.
- Use stable labels, `link(label(...))` and semantic references as appropriate;
  do not hand-number sections, captions or equations. Grapharch prefixes fragment
  anchors and footnotes to avoid collisions: preserve those identities.
- Typst math is not LaTeX. Check math-mode names and spacing rules against the
  compiler before translating equations. Bibliography keys and source assets must
  be supplied explicitly rather than invented to make a sample compile.

## Layout decisions

Shared defaults belong in `set`; element-specific transformations belong in
`show`. Reuse `ga-table`, `ga-code`, `ga-section`, `ga-callout`, `ga-example` and
`ga-cover` where applicable. Verify each helper's actual signature before use.
Make overrides at the layer that owns the style rather than repeating conditionals
in every emitted fragment.

Use explicit physical units for paper geometry and typographic sizes; use `fr`,
percentages and measured available width for flexible columns and content boxes.
Do not mechanically replace print dimensions with relative units.

For long tables, use semantic header cells/repeated headers and permit page
breaks. Check spanning cells and long IRIs. For code, preserve original text and
lexical context while wrapping grapheme clusters; do not truncate source to fit.
For headings and callouts, keep the heading with its first content while allowing
long bodies to break. For figures, preserve aspect ratio, caption, reference,
retained asset identity and supplied alt text.

Use `context` for location-dependent counters and queries. Avoid repeated scans
of all headings per page, uncontrolled state updates, and layout feedback loops.
Header/footer queries must behave on cover, front matter, chapter-opening and
continuation pages.

If `typstyle` is installed and supports the project's syntax, check touched
maintained `.typ` files. Inspect its diff before applying formatting; avoid
rewriting unrelated template code or generated documents merely for formatting.
