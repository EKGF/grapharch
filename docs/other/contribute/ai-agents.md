---
title: Working with coding agents
parent: Contribute
grapharch:
  subject: urn:grapharch:docs:topic:other:contribute:ai-agents
permalink: /other/contribute/ai-agents/
---

# Working with coding agents

This is the shared contributor entry point for Codex, Claude Code, and other
coding assistants. The repository, its tests, and its reviewed documentation
are the durable project context. A previous chat, a particular workstation,
private agent memory, and an installed skill package are not prerequisites.

## Read the project contract first

Start with the [architecture](../architecture/README.md) and its
[delivery contract](../architecture/deliverables.md). Consult the
[status page](../status.md) and the relevant usage guide before treating a
planned capability as implemented. The architecture documents contain the
project's design decisions; there is currently no separate numbered ADR series.
Do not infer GraphArch policy from instructions in a sibling repository.

The main implementation and reference locations are:

| Area | Location |
| --- | --- |
| Collection, documentation, planning and rendering | `src/publication/` |
| Documentation and publication vocabulary, validation shapes | `docs/other/ontology/` |
| Authoritative documentation sources | `docs/` |
| Executable examples and showcases | `examples/` |
| Regression tests and browser acceptance scripts | `tests/`, `tests/browser/` |
| CLI distribution and installer checks | `packaging/`, `action.yml` |
| CI and publication entry points | `.github/workflows/` |

The [ontology guide](../ontology/README.md) describes the vocabulary. Keep term
definitions, shapes, implementation and examples consistent when extending it.

## Preserve the stage boundaries

Collectors interpret source formats and native hints, retaining assertions,
content, dependencies and provenance in the RDF dataset. Documentors recognize
models in collected RDF and generate documentation fragments. Assembly validates
a revision; target resolution chooses content and placement. Renderers consume
that pinned plan and dataset, without reading the original source files again.

Keep a documented subject's identity separate from a fragment or its placement.
The same content can appear in several websites and PDFs without copied prose.
Navigation and visual styling do not change source ontology semantics or confer
approval status. Missing descriptions must not become invented definitions.

Use the existing contribution ownership and bounded execution mechanisms.
Replacing one source must not delete another source's data. Failed collection
must not promote a partial result or count as evidence that a source disappeared.
These are architectural requirements, not assistant-specific preferences.

## Build and verify locally

Follow the [development prerequisites](rust-install.md). The repository's
`rust-toolchain.toml` selects the tested Rust toolchain. For a source build:

```sh
cargo build --locked
cargo run --locked -- --help
```

Write a reproducing regression before correcting a defect. Run the affected
integration target first, for example:

```sh
cargo test --locked --test self_documentation
```

For shared pipeline changes, run the broader graph used by release validation:

```sh
cargo test --locked --all-targets --no-fail-fast
cargo clippy --locked --all-targets -- -D warnings
```

Formatting is checked with `cargo fmt --all --check`. Inspect the installed
hooks and `cog.toml` before a commit; use the normal hooks and do not bypass
them. The commit hook's default Clippy graph does not replace the all-targets
release check. Avoid repeating an identical check without a relevant change.

For installer or packaging changes, the existing package workflow runs:

```sh
python3 packaging/distribute.py check
python3 -m unittest discover -s packaging/tests
node --test npm/tests/*.test.cjs
```

Use the existing [examples](../../usage/examples.md) and regression fixtures
for the subsystem being changed. Tests should exercise observable behavior,
including export/import and rendering after the original sources are removed.
For HTML changes, inspect real generated pages at desktop and mobile sizes,
with keyboard interaction and relevant color schemes. For PDF layout changes,
render and inspect the affected pages; successful compilation alone does not
establish readable output. Report precisely which checks passed and which
remain incomplete.

## Work safely with other contributors

Inspect Git status before editing and before staging. Preserve unrelated edits
and untracked files; stage only the intended change. Coordinate shared files
and the Git index when several agents are active. Follow the normal
[contribution workflow](contribute-code.md) and
[commit conventions](git-commit.md).

A request in one conversation to push a branch, publish a release, or modify a
sibling project is not a standing permission for another contributor. Confirm
the scope of the current task; repository documentation does not supply release
credentials or publication authorization. Generated output is evidence to
review, not the authoritative source to patch.

For distribution work, follow the tracked `packaging/README.md` and the existing
release workflows. Keep the generator version and source provenance explicit.
Distinguish an implementation on `main`, a released binary, a downstream pin,
and a verified deployed publication. They are different delivery states.

## Skills and persistent knowledge

Local PDF, browser automation, or design skills can help with visual acceptance,
but they are optional tools. Their installation paths and private instructions
do not belong in source configuration. Equivalent browser and PDF tools can
perform the same checks; retain reusable acceptance scripts in this repository.
Never copy a private skill, credential, chat transcript, or machine-specific
handoff into the public documentation as project policy.

Record lasting design decisions and their rationale in the architecture
documentation, supported behavior in the usage guides and tests, and new
vocabulary in the ontology and shapes. Link those records from the relevant
guide. Keep temporary task status and local preview addresses outside the
durable design contract. When a new session starts, verify the current checkout
and test results rather than assuming that a previous session's report still
describes them.
