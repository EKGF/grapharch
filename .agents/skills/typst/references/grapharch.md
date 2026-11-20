# Pipeline and source map

Paths in this file are relative to the Grapharch repository root.

Read `docs/other/architecture/README.md` and `deliverables.md` in that directory
before structural work. `docs/features/targets/typst.md` describes intended PDF
behavior; `docs/usage/publication-families.md` and `docs/usage/bookshelves.md`
describe independent targets and large publications.

## Data and ownership

Collect → document → assemble revision → resolve target plan → render.
Renderers consume a resolved plan and content from the pinned KG revision. They
must work after exporting/restoring the dataset and removing original sources.
Do not reread Markdown, ontology files, or upstream repositories during rendering
to fill gaps. Find whether content or retained assets were lost earlier instead.

Specification HTML, companion website, and PDF have separate scopes, profiles,
placements, and destinations even when sharing a revision. Keep their reading
orders independent. Style is presentation: it cannot confer approval, invent
legal terms, supply a document identifier, or assert a release status.

## Implementation navigation

| Concern | Maintained source |
|---|---|
| Cover metadata, front matter, chapter emission | `src/publication/render/typst.rs` |
| Profile style selection and authority accent validation | `src/publication/render/typst_style.rs` |
| Shared typography, cover, running heads, table/code/callout helpers | `src/publication/render/typst/theme.typ` |
| OMG submission presentation overrides | `src/publication/render/typst/omg.typ` |
| Shared Markdown validation and string quoting | `src/publication/render/markup.rs` |
| Markdown events to Typst | `src/publication/render/typst_markdown.rs` |
| Native HTML fragment conversion | `src/publication/render/native_typst.rs` |
| Syntax-highlighted code | `src/publication/render/code.rs` |
| Retained images and print selection | `src/publication/render/collected_images.rs`, `image_variants.rs` |
| Embedded compiler, fonts, World and PDF encoding | `src/publication/render/engine.rs` |
| Executable showcase entry point | `examples/publish_showcase.rs` |

The older `src/generator/typst/` path also exists. Establish the actual caller
before touching it; the publication engine above is the KG-backed path.

## Existing presentations

Inspect `Style::read` for precedence; renderer selection can override the profile.
Currently recognized profile styles are `reader`, `specification`, `overview`,
and `omg`. Unsupported styles are errors.

- Reader uses slate/teal, shared A4 geometry, and reference-friendly typography.
- Specification uses the publisher's validated accent and publication metadata.
- Overview uses indigo/orange, larger chapter openings, and a relaxed measure.
- OMG uses restrained black typography, US Letter geometry, and its own cover
  and footer overrides after the shared theme. The skin implies no approval.

Check header/footer interactions, numbering and outline membership independently.
Publisher preface, legal preamble, ordinary chapters, and quality appendices have
different roles; do not fix their placement by injecting arbitrary page breaks
into source prose.

## Embedded compiler invariants

`engine.rs` currently disables system-font discovery and adds bundled Liberation
Sans to Typst's embedded font assets. A font available to the CLI may be absent
from Grapharch. Check the embedded font book before choosing a family.

The World reads generated files beneath the publication root and rejects package
FileIds. `@preview` imports that work in the CLI therefore do not establish native
support. Do not add package fetching as a layout workaround.

The World supplies no current date; generated document metadata sets `date: none`.
Keep dates source-owned and explicit. Compiler warnings currently fail native
publication. Fix their cause rather than suppressing them.

Compilation runs on semaphore-bounded blocking workers. Preserve that audited
synchronous compiler boundary; ordinary pipeline filesystem work remains async.
Do not hydrate the KG to simplify rendering. Running headings already avoid an
interpreted full chapter scan per page; retain that performance property.
