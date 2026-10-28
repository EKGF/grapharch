# GraphArch

[![Docs CI](https://github.com/EKGF/grapharch/actions/workflows/pages.yml/badge.svg?branch=main)](https://github.com/EKGF/grapharch/actions/workflows/pages.yml)
[![Build](https://github.com/EKGF/grapharch/actions/workflows/rust-ci.yml/badge.svg?branch=main)](https://github.com/EKGF/grapharch/actions/workflows/rust-ci.yml)
[![Documentation](https://img.shields.io/badge/docs-website-116659)](https://ekgf.github.io/grapharch/)
[![License](https://img.shields.io/github/license/EKGF/grapharch)](LICENSE)

Turn authored text, ontologies, shapes, and related RDF into a documentation
knowledge graph, then publish a specification, companion website, or PDF.

GraphArch is one command-line tool. Keep your content and publication
configuration in your project; use the same command locally and in CI.

## Get started

Use a prebuilt release for normal use. Download the archive for your platform
from [GitHub Releases](https://github.com/EKGF/grapharch/releases), extract it,
and put `grapharch` (or `grapharch.exe`) on your `PATH`.

Standalone binaries are published on GitHub Releases. Package-manager channels
have separate publication requirements; see the installation guide for their
availability.

To build in GitHub Actions:

```yaml
- uses: actions/checkout@v7
- uses: EKGF/grapharch@v0.1.0
  with:
    version: '0.1.0'
    source-kind: checkout
```

See [GitHub Actions](https://ekgf.github.io/grapharch/usage/github-actions/)
for configuration and Pages publication.

After installation:

```sh
grapharch --version
grapharch publication --help
```

Follow [Getting started](https://ekgf.github.io/grapharch/usage/getting-started/)
for installation options and your first publication. See the
[showcase gallery](https://ekgf.github.io/grapharch/showcases/) for generated
HTML, PDFs, and downloadable documentation graphs.

## Publish your documentation

GraphArch can read a directory, a local Git checkout including your edits, or a
remote Git repository. Your publication definitions select the content,
publishing authority, style, and outputs.

- [Publish an ontology family](https://ekgf.github.io/grapharch/usage/ontology-families/).
- [Publish a specification, companion website, and PDF](https://ekgf.github.io/grapharch/usage/publication-families/).
- [Generate a documentation website](https://ekgf.github.io/grapharch/usage/self-hosting/).

The same pipeline works with your own ontologies, repositories, and publishing
authority. EKGF ontologies and DPROD are examples, alongside the other
[executable showcases](docs/usage/examples.md).

## Build from source

This is for contributors and people testing unreleased changes. Install the
[development prerequisites](docs/other/contribute/rust-install.md), then run:

```sh
git clone https://github.com/EKGF/grapharch.git
cd grapharch
cargo build --release --locked
./target/release/grapharch --help
```

On Windows, run `target\release\grapharch.exe --help` after the build. See
[Contributing](CONTRIBUTING.md) for development guidance and the
[implementation status](docs/other/status.md) for current limits.

## License

GraphArch is available under the [MIT license](LICENSE). Bundled reference
sources and fonts retain their own licenses.
