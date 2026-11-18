# Agent instructions

These instructions apply throughout this repository. Read more specific
instructions in a directory before changing files there.

## Working rules

- No compliments. Communicate plainly and report evidence and limitations.
- Find root causes. Do not substitute symptom fixes or workarounds.
- Breaking changes are welcome when they improve the design. Do not retain
  compatibility layers merely to avoid a breaking change.
- Fail fast on invalid input, unsupported behavior, and violated invariants.
  Return explicit errors; do not silently skip data, invent content, or add
  fallback behavior. Honor explicitly configured diagnostic collection modes.
- For a bug, write a regression test first, demonstrate its failure, then fix
  the implementation until the test passes.
- Preserve unrelated working-tree changes. Other contributors may be editing
  this repository or sibling repositories concurrently.
- Do not commit, push, or publish externally unless requested.

## Architecture

Read [the architecture](docs/other/architecture/README.md) and
[the deliverables](docs/other/architecture/deliverables.md) before structural
changes. Treat architectural decision records as authoritative if present;
surface conflicts instead of silently disregarding them.

- Distribute one CLI, with specialized collectors, documentors, and renderers
  behind explicit interfaces.
- Keep each collector small and focused on one kind of documentable input.
  Collectors interpret source-specific hints and contribute RDF with provenance.
- Run independent collectors concurrently, stream bounded output through
  channels, and respect each store's writer concurrency constraints.
- Documentors consume collected RDF to generate documentation fragments.
- Renderers consume resolved publication plans pinned to a KG revision. They
  must not reread original sources or derive publication content outside the KG.
- Keep ontology identities distinct from documentation component identities.
- Preserve provenance, stable identities, deterministic ordering, and isolation
  between source contributions. Never replace unrelated graphs.
- Support distinct specification, companion website, and PDF publication targets
  without coupling their presentation or inventing source descriptions.
- Record diagnostics in the KG against the observed source revision. Distinguish
  warnings from failed operations.
- Keep execution bounded. Do not introduce unbounded queues, whole-dataset
  buffering, repeated per-link graph queries, or implicit network import fetching.

## Repository conventions

- Publication implementation lives in `src/publication/`.
- The documentation ontology and validation shapes live in
  `docs/other/ontology/`.
- Documentation sources live in `docs/`; executable showcases live in
  `examples/`; regression tests live in `tests/`.
- Showcases also serve as acceptance tests. Verify that rendering works from a
  restored exported KG after removing the original source files.
- Keep copied example sources reproducible. Record their origin, revision or
  working-tree state, license, and content hashes. Never describe uncommitted
  sources as identical to a committed revision.
- Generated artifacts belong under ignored output directories. Fix source files
  and regenerate; do not rely on hand-edited generated output as the solution.
- Do not modify sibling repositories without authorization.

## Verification

Run the relevant regression tests and checks for the change:

```sh
cargo test --test <relevant_integration_test>
cargo fmt --check
cargo clippy --all-targets -- -D warnings
git diff --check
```

Use `cargo test --all-targets` for changes affecting shared pipeline behavior.
For UI changes, check the generated page in a browser, including keyboard and
responsive behavior. For PDF layout changes, render and visually inspect the
affected pages. Report what was checked and any remaining limitations.

## Shell scripts

Never write `set -e`, `set -eu`, `set -euo pipefail`, or `set -o errexit`.
Check each command's exit status explicitly at its call site with `if`, an
explicit status variable, or a failure handler. Ignore a failure only when it is
genuinely harmless, with a comment explaining why. `set -u` and
`set -o pipefail` on their own are allowed.

The shell disables errexit in several conditional contexts, including functions
called from those contexts. It therefore cannot provide reliable error coverage.
