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
installs and verifies the GraphArch release it belongs to before running the
build.

## Build in one step

Reference the action by the full commit SHA of a GraphArch release, with the
release tag as a comment. After checking out your repository, add this step:

```yaml
- uses: actions/checkout@v7
- uses: EKGF/grapharch@<release-commit-sha> # vX.Y.Z
  id: docs
  with:
    source-kind: checkout
    source: .
    output: build/docs
```

The reference alone selects the generator. The action reads the version from
its own `Cargo.toml`, takes its own commit from the reference, and installs the
native binary of that release. Before running it, the action checks the
published archive against its `.sha256` file and the release manifest against
the version, platform, archive checksum and source commit. The manifest's
`source_commit` must be the action's commit. A branch, a tag name, a short SHA,
or a commit that is not the release commit of its version fails the action. It
never compiles a replacement or downloads another release. Self-selection is
available from the first release after v0.2.8; earlier releases need `version`
and `expected-revision`.

Use the commit of the release tag, not the annotated tag object. The release
page shows it, or run:

```sh
git ls-remote https://github.com/EKGF/grapharch 'refs/tags/vX.Y.Z^{}'
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

For installation alone, set `mode: install`; the `grapharch` command is then
available to subsequent steps:

```yaml
- uses: EKGF/grapharch@<release-commit-sha> # vX.Y.Z
  id: grapharch
  with:
    mode: install
```

Both modes expose `version`, `revision` and `binary-path`. `revision` is the
verified release source commit. Deployment is a separate choice.

## Upgrade with Dependabot

Because one line selects both the action and its binary, Dependabot upgrades
GraphArch with a single change. Enable version updates for GitHub Actions in
`.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
```

For a new release, Dependabot replaces the commit SHA and the `# vX.Y.Z`
comment together. Review and merge the pull request; the action then installs
the new release. Each automatic release tags its own release commit, so every
`vX.Y.Z` tag commit carries the action and the `Cargo.toml` of that release.

## Install from a checked-out release

A workflow can also check out GraphArch at a release commit, for example when a
configuration file records the generator revision, and run that checkout as a
local action:

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
```

A local action has no reference, so it identifies itself by the `HEAD` commit
of its checkout. The action path must be the root of that Git checkout, and the
checkout must be clean, with no modified, untracked or extra ignored executable
inputs. The same release verification then applies: the `HEAD` commit must be
the release commit of the checked-out version.

## Install another release explicitly

Supply `version` and `expected-revision` together to install a release other
than the action's own:

```yaml
- uses: EKGF/grapharch@v0
  with:
    mode: install
    version: 'X.Y.Z'
    expected-revision: <release-commit-sha>
```

The action verifies that release against `expected-revision` exactly as it
verifies its own. Use this when the action reference is not a release commit:
the floating `v0` and `v0.2` references, a checkout of an unreleased commit, or
a composite action of your own that wraps this one. Within a wrapping composite
action, the runner reports the outer action's reference instead of this one
([actions/runner#2473](https://github.com/actions/runner/issues/2473)), so
self-selection fails there. `version` without `expected-revision`, or the
reverse, fails before anything is downloaded.

An automatic release advances the Action convenience tags `vMAJOR` and
`vMAJOR.MINOR` only after all its publication channels succeed.
`EKGF/grapharch@v0` follows the maintained Action implementation within major
version zero. Because these references move, they never select a binary
themselves. Existing repository workflows are never rewritten by a release.

## Install an unreleased source revision

On Linux runners, the same action can build an exact reviewed source revision.
Check out GraphArch separately, then invoke that local action with
`source-revision`:

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
directory and do not use the release binary cache. `source-revision` excludes
`version` and `expected-revision`; an invalid selector or failed build fails the
action. Every selection supports `mode: install` and the normal family build
mode.

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
commit and source location. A case also fails when a relative `href` or `src` in
its published HTML does not name a file that the same case publishes, such as a
PDF edition that was not built. Absolute links are not fetched.

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
