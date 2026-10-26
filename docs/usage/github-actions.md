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

> [!IMPORTANT]
> The first native release is still being prepared. These workflows require a
> published release supporting the publication JSON report; `0.0.1` below is the
> planned initial version, not a currently available download.

## Build in one step

After checking out your repository, add this step:

```yaml
- uses: actions/checkout@v7
- uses: EKGF/grapharch@7f6703989bba63143c1c063cdd2052cf1821cebc
  id: docs
  with:
    version: '0.0.1'
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
The KG archive is exposed separately. A failed build fails the step.

For installation alone, set `mode: install` and supply only `version`; the
`grapharch` command is then available to subsequent steps. Both modes expose
`version` and `binary-path`. Deployment is a separate choice.

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
    uses: EKGF/grapharch/.github/workflows/publish-family-pages.yml@92f22d5a653876e9dee1f50c4543a688ba8c9f97
    with:
      version: '0.0.1'
      source-kind: checkout
      source: .
      deploy: false
```

Pin the reusable workflow to the reviewed commit that contains it. Its internal
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
