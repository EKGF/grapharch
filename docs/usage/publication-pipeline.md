---
title: Publication pipeline foundation
layout: default
parent: Usage
nav_order: 2
grapharch:
  subject: urn:grapharch:docs:topic:usage:publication-pipeline
permalink: /usage/publication-pipeline/
---

# Publication pipeline foundation

The `publication` commands operate on the new graph-backed foundation. They
support assembly, target planning, static HTML, and embedded Typst/PDF output.
They do not yet implement the complete DPROD release. See
[implementation status](../other/status.md) and
[the delivery contract](../other/architecture/deliverables.md).

Run commands from the repository root. Validate the generic ontology-family,
use-case-tree, and portfolio definitions:

```bash
cargo run -- publication validate --definitions tests/fixtures/publication/projects.ttl
```

Collect those RDF definitions into a persistent shared dataset:

```bash
cargo run -- publication collect --store /tmp/grapharch-example --project urn:project:fixtures --collector urn:collector:definitions --kind rdf --source tests/fixtures/publication/projects.ttl
cargo run -- publication snapshot --store /tmp/grapharch-example
```

The second command prints an immutable snapshot IRI. Copy that IRI into the
explicit `--snapshot` argument below:

```bash
cargo run -- publication resolve-scope --store /tmp/grapharch-example --project urn:project:fixtures --snapshot SNAPSHOT_IRI --scope urn:example:family
```

Scope results are stored as a contribution in the same dataset. The command
prints its plan IRI. Its input snapshot stays pinned even if source collection
later changes. A snapshot captures complete contribution heads; it is not yet an
assembled, validated publication revision.

## Assemble and resolve publication targets

Assembly selects only the requested project's collected/documented contributions.
Plans are excluded, so resolving targets does not change the next source revision.
Schema validation reads the pinned logical data, not inactive or staging graphs.
A guarded promotion rechecks pending/failed inputs and current contribution heads;
an invalid edit cannot replace the last validated revision.

The generic fixture declares `urn:example:ontology-project`, rather than the
container identity `urn:project:fixtures` used in the inspection example above.
For a publication project, collect its definitions under that declared project:

```bash
cargo run -- publication collect --store /tmp/grapharch-publication --project urn:example:ontology-project --collector urn:collector:definitions --kind rdf --source tests/fixtures/publication/projects.ttl
cargo run -- publication assemble --store /tmp/grapharch-publication --project urn:example:ontology-project
cargo run -- publication plan --store /tmp/grapharch-publication --project urn:example:ontology-project --revision VALIDATED_REVISION_IRI --all
```

Use the revision IRI printed by `assemble`. Alternatively select one target with
`--target urn:example:spec`; `--target` and `--all` are mutually exclusive. Planning
prints a `plan=...` identity and `snapshot=...` immutable plan view for each target.
The plan view stays readable even after another run replaces the target's head.
Historical validated source revisions remain selectable while newer drafts are
invalid.

Current placement rules are explicit: profile placement orders must be distinct
nonnegative `xsd:integer` literals; multiple selected fragments in one component require
explicit distinct fragment orders. A single selected fragment needs no extra
order hint. Fragment subjects must fall within the resolved scope. Required
references must resolve to selected profile content within that scope or to an
explicit `ExternalReference`. A plain Markdown/SPARQL file without a subject hint
documents its stable source artifact identity.

Destination directories currently must be relative, without traversal or dot
components. Equal or nested target destinations fail until explicit composition
is implemented. Publication profiles and reference explanations are still incomplete.

## Render a pinned plan

Use the plan and snapshot printed by `plan`:

```bash
cargo run -- publication render --store /tmp/grapharch-publication --plan PLAN_IRI --snapshot PLAN_SNAPSHOT_IRI --output-root output/publications
```

Rendering reads the immutable plan and validated source revision from the dataset;
it does not reopen original source files. HTML targets produce `index.html` and
`style.css`; Typst targets produce `main.typ` and `document.pdf`. Each target
includes `manifest.yaml` recording its inputs and generated files. Embedded fonts
and the Rust Typst compiler require no system Typst installation or network.

Run the complete three-target demonstration:

```bash
cargo run --example publish_showcase
```

The example writes specification HTML, reader HTML, and a review PDF under
`output/pdf/grapharch-showcase`, together with an exported dataset. It demonstrates
cross references, tables, code, a glossary, references, and an appendix.

Run the mixed-source showcase with `cargo run --example publish_showcase -- --mixed`.
It collects separate Markdown, SPARQL, and OWL files, generates an OWL class
description, and publishes HTML and PDF. Both showcases share their execution
code with `tests/showcases.rs`. See the [showcase catalog](examples.md)
for assertions, explicit fixture wiring, and current limits.

Profiles support `ga:theme ga:SpecificationTheme` or `"reader"` for HTML. Components use
`ga:sectionKind "section"`, `"appendix"`, `"glossary"`, or `"references"`.
Titles use `dcterms:title`. Markdown heading levels are relative to each fragment's
shallowest heading level; skipped heading levels fail. Internal subject links resolve
to stable publication anchors. External references may supply `ga:url`.

Supported bodies are Markdown, plain text, SPARQL, Turtle, and N-Triples. Raw HTML,
images, executable MDX, and unsupported Markdown constructs fail explicitly.
Relative asset links require an asset collection implementation and currently fail.
The renderer caps each fragment at 1 MiB, each target at 10,000 entries and 16 MiB
of body text, and tables at 32 columns. These are resource bounds, not performance
acceptance measurements.

`Publisher` clones share bounded rendering and compilation workers. Requests carry
monotonic sequence numbers assigned before future polling; superseded work cannot
replace a newer request. Output is staged privately before directory promotion.
Native file locks serialize filesystem promotion and release on process exit.
Interrupted directory promotion is recovered on the next request. Existing
unowned directories and extra files fail ownership checks and are preserved.
Publication jobs and their outcomes are recorded in `urn:grapharch:publications`
without changing source contribution heads. A watcher and editor integration are
still outstanding.

For standards-style publication front matter and numbered navigation, use the
[specification renderer](specification-renderer.md) with `ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer>`.

## Export and restore

Export and restore into a new, empty dataset without the original sources:

```bash
cargo run -- publication export --store /tmp/grapharch-example --output /tmp/grapharch-example.nq
cargo run -- publication import --store /tmp/grapharch-restored --input /tmp/grapharch-example.nq
```

Imports reject nonempty datasets. Export includes ownership metadata, retained
versions, and snapshots. Large-import transaction memory behavior still needs
scale verification before D2 can be considered complete.

## Specialized adapters

`--kind markdown`, `--kind sparql`, `--kind rdf`, and `--kind w3c-specification`
select separate collectors. The last kind requires `--source-url` and reads a
pinned, already-published W3C HTML file; see the
[specification renderer](specification-renderer.md).
Project, collector-instance IRI, and source identity jointly determine ownership.
Use stable source paths across invocations; changing the path changes the source
identity. A later project configuration layer will define repository-relative
source identities and source-set reconciliation.

Markdown supports an optional title and one normalized subject hint:

```markdown
---
title: Introduction
grapharch:
  subject: urn:domain:ontology
---
# Introduction

Authored documentation.
```

Other frontmatter fields are left uninterpreted. Unsupported keys inside the
`grapharch` mapping fail explicitly. MDX component execution is not implemented.

SPARQL files support `# grapharch:subject urn:domain:subject`. Statements are
stored as documentable content and are never executed by the collector.

RDF input uses its file extension to select a supported parser. Named graphs
are preserved through contribution mappings. Blank nodes are deterministically
skolemized by project, collector, source, source revision, and encounter order;
this guarantees identical source bytes recollect identically, not graph
canonicalization across differently serialized RDF.

Authored files are capped at 256 KiB, RDF files at 8 MiB. Each producer buffers
at most 256 quads and 1 MiB of accounted chunk data. The writer separately bounds
queued messages and bytes, including metadata. These are initial explicit limits,
not measured performance claims. Every attempt is registered before source I/O;
parse or read failures preserve the previous contribution and block readiness
checks until the source is successfully recollected or explicitly removed.

## Library interfaces

- `Collector::produce` streams normalized assertions into `ContributionStream`;
  the shared `collect` lifecycle registers, finalizes, or aborts the attempt.
- `collect_all` schedules a lazy collector iterator with bounded concurrency.
- `GraphWriter` owns the embedded backend's serial write queue; `write` splits
  output on both byte and quad counts, while `push` accepts already bounded chunks.
- `Dataset` supports memory/disk storage, source attempts, immutable snapshots,
  streaming inspection SELECT results, and dataset export/import.
- `Dataset::logical_pattern` scopes triple patterns to a pinned snapshot so
  inactive and staging data cannot enter publication queries.
- `Assembler` validates a project revision and records a guarded current pointer.
- `TargetPlanner` records target-specific entries, validates references and output
  directory separation, and returns an immutable plan snapshot.
- `Publisher` renders a pinned plan with bounded workers and guarded artifact promotion.
- `ScopeResolver` uses graph-backed working sets for membership and traversal.
  Composition cycles fail; domain relationship cycles terminate through visited
  membership. Composition nesting has an explicit 64-level limit.
- Specialized OWL/SHACL documentor types generate descriptions and assertion
  fragments. The OWL-class path has behavioral coverage, including removal of
  obsolete generated fragments while preserving historical snapshots. The other
  documentor types and complete SHACL interpretation need additional acceptance
  coverage. Multiple comments currently require explicit selection and fail.

The old `generate` CLI and original source/book/Typst prototype remain separate.
No command above deploys a website or claims OMG template compliance.

### Execution policy and diagnostic fragments

`--fail-fast` is the default; `--keep-going` explicitly requests continued
independent work. The flags are global and mutually exclusive. Collector batches
expose the same policy through `collect_all_with_mode`, returning completed-input
and error counts. A failed source still blocks assembly of a clean revision.

Collection and publication failures are retained as `ga:Diagnostic` resources,
with severity, stable code, message, reference, affected fragment when known,
operation provenance, and a pinned input revision. Each diagnostic has its own
`ga:Fragment` containing Markdown, linked through `ga:fragment`. Collection errors
live in `urn:grapharch:diagnostics`; publication jobs and their diagnostic fragments
live in `urn:grapharch:publications`. These derived graphs do not advance source
contribution heads or change the source revision being diagnosed.

The specification renderer includes an errors addendum in HTML and PDF when
explicit keep-going rendering encounters supported reference errors. Its manifest
records diagnostics and execution policy; the job becomes `published-with-errors`.
The CLI exits unsuccessfully even when a diagnostic preview was produced. Fatal
parsing, storage, or compilation failures remain hard errors.

The dedicated specification renderer produces `index.html`, `style.css`,
`main.typ`, and `document.pdf` from the same pinned plan. The HTML PDF icon links
to that PDF. Publication stages and promotes the paired artifacts together.

## Exact-revision builds

A trusted central workflow can publish a designated branch of a repository
without a showcase registry, without executing code from that branch, and
without the branch carrying any GraphArch configuration. One controlling
branch holds every GraphArch definition. GraphArch runs once per designated
branch: definitions come from a checkout of the controlling branch, and only
content comes from the designated branch, at an exact commit:

```sh
grapharch publication build-revision \
  --git-repository https://github.com/EKGF/dprod.git \
  --git-commit <exact-content-commit> \
  --definitions controlling/publication/definitions/develop.ttl \
  --definitions-commit <exact-controlling-commit> \
  --project urn:dprod-showcase:project \
  --all-targets \
  --generator-revision <exact-generator-commit> \
  --publication-url https://ekgf.github.io/dprod/spec/develop/ \
  --git-cache output/git-cache \
  --output-root output/publication --json
```

All three revision arguments require full lowercase Git commit IDs; branch
names and abbreviated IDs are rejected before any network access. The
definitions checkout is verified strictly before the content is fetched: its
`HEAD` must be `--definitions-commit`, the file must be tracked and committed
there, and the working file must be identical to the committed blob. The
definitions are then read from the checkout's object database, never from the
working tree. Content is fetched into the bare cache and read as data only.

Select targets with one or more `--target <IRI>` options, published in the
given order, or with `--all-targets`, which publishes every target the
project registers in IRI order. Exactly one form is required.

Where the output is deployed is a build input, never source content.
`--publication-url` names the deployed URL of the output root: an absolute
HTTP(S) directory URL ending in `/`, without credentials, query or fragment.
Each selected target's `ga:thisVersion` is derived from it and the target's
`ga:destination`: the destination directory, with a trailing `/`, for HTML
renderers (specification, website, HTML, specification pages and bookshelf),
and the destination's `document.pdf` for a Typst target. With the URL above, a
specification at `specification` is `…/spec/develop/specification/` and a PDF
at `pdf/ekgf` is `…/spec/develop/pdf/ekgf/document.pdf`. A definitions file
that declares `ga:thisVersion` on a selected target is rejected as a conflict
with the derived value. Other version links, such as `ga:latestVersion` and
`ga:editorDraft`, remain authored metadata, and authored links between
co-built targets stay relative to the page. The same output therefore works at
any deployment base the orchestrator chooses.

The definitions name every collector use with `ga:collector`, each reading
one `ga:sourcePath` of the content tree; the content cannot add collectors.
A content contribution that asserts publication definition vocabulary, such
as `ga:target`, `ga:placement`, `ga:renderer` or a `ga:Target`, or that
describes a resource the definitions declare, is rejected: only the
controlling definitions define publications. `ga:includedIn` places the
subjects of a collector use's `ga:fragment` in a scope. It is required on
every collector use that names a `ga:fragment`, including Markdown files and
pages, code examples and legal preambles: a fragment outside every scope would
be collected and never published, so a missing inclusion fails the build. A
`ga:VocabularyReferenceDocumentor` use generates term fragments into the
components of its authored `ga:ReferenceSection` resources; placements are
authored in each profile. `ga:metadataSource` and `ga:derivedMetadata` derive
header values from the native source's own contribution:

```turtle
doc:specification ga:metadataSource doc:source:ontology ;
    ga:derivedMetadata dct:title, dct:description, ga:license, ga:publicationDate .
```

The mapping is fixed: `dct:title` and `dct:description` from the same
predicates, `ga:license` from `dct:license`, and `ga:publicationDate` from
`dct:issued`. A missing or multi-valued native value is an error, and so is an
explicit definition value for a derived predicate. A source without an
issuance date instead declares an explicit `ga:publicationDate` on its targets
and does not derive it; this describes the publication without adding an
issuance assertion to the source ontology.

Every selected target is rendered from a restored `documentation.nq`. The new
output directory appears only after all targets succeed; a failed build
removes its staging directory, and an existing output is rejected.
`build-provenance.json` (version 2) records the project, the target selection,
the publication URL, the content repository and commit, the definitions repository, path, commit,
blob and SHA-256 digest, the declared generator revision with its version and
executable digest, the graph file, and each publication manifest with its
destination.
