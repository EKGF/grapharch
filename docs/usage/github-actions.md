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
- uses: actions/checkout@v4
- uses: EKGF/grapharch@3dd1d4b21ff897b5761f2ee1e31e10ec97f3e9c8
  id: docs
  with:
    version: '0.0.1'
    source-kind: checkout
    source: .
    definitions: grapharch.ttl
    project: urn:example:project
    target: urn:example:specification
    output: build/docs
```

Replace the definitions path and both IRIs with your project's publication
configuration. `source-kind: checkout` records provenance from a local Git
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
    uses: EKGF/grapharch/.github/workflows/publish-family-pages.yml@<reviewed-workflow-commit-SHA>
    with:
      version: '0.0.1'
      source-kind: checkout
      source: .
      definitions: grapharch.ttl
      project: urn:example:project
      target: urn:example:specification
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
