---
title: GitHub Actions
parent: Usage
nav_order: 8
permalink: /usage/github-actions/
grapharch:
  subject: urn:grapharch:docs:topic:usage:github-actions
---

# Build documentation in GitHub Actions

Keep your publication configuration with your ontologies, then use one GraphArch
step to build their specification website, PDF and documentation KG. The action
installs and verifies an exact release before running the build.

Pin both the Action revision and the native binary version when upgrading.
The example below selects an existing release; it is a reproducible version pin,
not a promise to track the newest generator automatically.

## Build in one step

After checking out your repository, add this step:

```yaml
- uses: actions/checkout@v7
- uses: EKGF/grapharch@v0.2.4
  id: docs
  with:
    version: '0.2.4'
    source-kind: checkout
    source: .
    output: build/docs
```

GraphArch discovers a unique project and target from publication hints in your
repository’s RDF. Put those hints in your ontologies or a separate RDF file
alongside them. Ambiguous or incomplete hints fail explicitly. To use external
definitions instead, supply `definitions`, `project` and `target` together.

`source-kind: checkout` records provenance from a local Git
checkout. Use `directory` to collect a plain source directory; GraphArch never
switches between these modes implicitly. Paths with spaces are supported. Keep
generated output directories ignored in your repository when also building
locally.

The step exposes `publication-path`, `pdf-path` and `kg-path` as absolute paths.
The publication path is the website directory containing `index.html`. Use
these outputs to upload artifacts or integrate your own deployment process.
The KG archive is exposed separately. `pdf-path` is empty for paginated or bookshelf publications without a single collection PDF; book PDFs remain inside the publication directory. A failed build fails the step.

For installation alone, set `mode: install` and supply only `version`; the
`grapharch` command is then available to subsequent steps. Both modes expose
`version` and `binary-path`. Deployment is a separate choice.

For a released generator whose exact source revision is part of your publication
record, also supply `expected-revision` with the release commit SHA. The action
checks the published platform manifest against that commit, the version, target,
and verified archive checksum before running the binary. Its `revision` output
then records the verified release source commit. Missing or mismatched provenance
fails the action; it never switches to a source build.

An automatic release advances the Action convenience tags `vMAJOR` and
`vMAJOR.MINOR` only after all its publication channels succeed. Once available,
`EKGF/grapharch@v0` follows the maintained Action implementation within major
version zero. The `version` input still selects an exact published binary;
changing the Action reference does not silently upgrade that binary. Use an
exact release tag or full commit SHA when the Action implementation must also
remain immutable. Existing repository workflows are never rewritten by a release.

## Install an unreleased source revision

On Linux runners, the same action can build an exact reviewed source revision.
Check out GraphArch separately, then invoke that local action with
`source-revision` instead of `version`:

```yaml
- uses: actions/checkout@v7
  with:
    repository: EKGF/grapharch
    ref: ${{ steps.configuration.outputs.generator-revision }}
    path: .grapharch-tool
    persist-credentials: false
- uses: ./.grapharch-tool
  id: grapharch
  with:
    mode: install
    source-revision: ${{ steps.configuration.outputs.generator-revision }}
```

The configuration output must contain the full 40-character Git commit SHA.
The action verifies that its checkout matches and has no modified or extra
executable inputs, installs Linux compiler dependencies and the repository's
pinned Rust toolchain, then builds with `cargo build --locked --release`.
The runner must provide Git, rustup, and sudo access to apt-get, as standard
GitHub-hosted Ubuntu runners do. Source builds are not supported on macOS or
Windows yet; release installation remains available there.

`binary-path` and `version` retain their existing meanings; `revision` records
the exact source commit. Source builds use a separate runner-temporary build
directory and do not use the release binary cache. Supply exactly one of
`version` or `source-revision`; an invalid selector or failed build fails the
action. Both selectors support `mode: install` and the normal family build mode.

## Build a Pages artifact

The reusable workflow checks out your repository, builds the family, and stages
the generated website with a downloadable `documentation.nq`. It uploads a
Pages artifact without deploying by default. Start with a manual workflow:

```yaml
name: Documentation
on:
  workflow_dispatch:
permissions:
  contents: read
  pages: write
  id-token: write
jobs:
  documentation:
    uses: EKGF/grapharch/.github/workflows/publish-family-pages.yml@v0.2.4
    with:
      version: '0.2.4'
      source-kind: checkout
      source: .
      deploy: false
```

For an immutable pin, replace `v0.2.4` with the release commit SHA after reviewing it. Its internal
GraphArch step is also pinned to a reviewed commit. The workflow reads the
publication paths returned by the build; it does not recollect sources to
assemble the deployment artifact.

After inspecting the artifact, enable GitHub Pages with **GitHub Actions** as
the publishing source and set `deploy: true`. The deployment uses the
`github-pages` environment and returns a `page-url` workflow output. Set
`include-kg: false` to omit the downloadable KG. You can add branch triggers
when you are ready for automatic publication.

## Check live showcases in CI

GraphArch’s own gallery is also an integration test suite. Run every declared
showcase and retain evidence when an upstream repository changes:

```sh
grapharch publication verify-gallery \
  --source examples --project urn:grapharch:docs \
  --output-root output/showcase-verification --json
```

Use a new or empty output directory. Verification checks every case with bounded
parallelism, records unexpected failures as KG diagnostics, and exits nonzero if
any case fails. It retains `report.json`, `verification.nq`, and each available
case’s artifacts and KG. A collection failure may prevent a case KG from being
available; the report says so explicitly. Repository failures include the observed
commit and source location.

The GraphArch website workflow runs these live checks on changes and nightly.
It uploads the evidence even when verification fails and blocks publication of
an incomplete gallery. Pinned offline fixtures remain separate regression tests;
they are never substituted for an unavailable live source.

## Local UML rendering dependency

The action explicitly installs Temurin Java 21 and PlantUML MIT 1.2026.9 during
setup, including `mode: install`. The official release JAR is pinned by SHA-256
in `packaging/setup_plantuml.py`; cached bytes are rechecked before use. Setup
verifies the reported version and renders a small Smetana class diagram before
adding the launcher to PATH. Unsupported platforms, checksum mismatches and
missing Java fail setup.

OWL diagram generation needs this local renderer. Smetana runs inside Java and
does not require Graphviz. Authored PlantUML diagrams that explicitly use other
layout engines may have additional prerequisites. The CLI does not download a
renderer or send ontology content to an external service. Rendering an already
collected and documented KG requires neither Java nor PlantUML.

Repository CI uses `.github/actions/setup-plantuml`, which calls the same
installer. Direct CLI users must install Java and PlantUML on PATH themselves.
The JAR is an explicit CI tool dependency, not bundled into GraphArch binaries.
