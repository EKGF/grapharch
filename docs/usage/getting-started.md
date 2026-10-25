---
title: Getting Started
layout: default
parent: Usage
nav_order: 1
grapharch:
  subject: urn:grapharch:docs:topic:usage:getting-started
permalink: /usage/getting-started/
---

# Getting started

Install a prebuilt GraphArch release, keep a small publication configuration in
your project, and run the same command locally and in CI.

> [!IMPORTANT]
> Release binaries and package-manager packages are being prepared and are not
> published yet. The commands below describe those installation channels; use
> them once a release is listed. To evaluate the current development version,
> follow [Build from source](#build-from-source).

## Install a prebuilt binary

Download the archive for your platform from
[GitHub Releases](https://github.com/EKGF/grapharch/releases). Each archive has an
adjacent SHA-256 checksum file. Verify the download, extract it, and put
`grapharch` (or `grapharch.exe` on Windows) in a directory on your `PATH`.

The planned release platforms are:

| System | CPU | Requirement |
| --- | --- | --- |
| Linux | x64 or ARM64 | glibc 2.28 or newer |
| macOS | Intel or Apple silicon | macOS 11 or newer |
| Windows | x64 or ARM64 | Native Windows executable |

Musl-based Linux distributions and 32-bit systems are not included in this
release matrix. Select the archive for the system where GraphArch will run.

### Other installation channels

These channels install the same CLI. They are pending publication; choose the
package manager already used by your project once its package is available.

| Channel | Install or run |
| --- | --- |
| Homebrew | `brew install legra-ai/tap/grapharch` |
| Python with uv | `uv tool install grapharch`, or `uvx grapharch --help` for an isolated run |
| Python with pip | `python -m pip install grapharch` |
| npm project dependency | `npm install --save-dev grapharch`, then `npx grapharch --help` |
| cargo-binstall | `cargo binstall grapharch` |

For a repeatable CI build, pin an exact released version instead of tracking the
latest package. A missing binary or unsupported platform is an installation
error; these installers do not silently compile GraphArch from source.

## Check the installation

```sh
grapharch --version
grapharch publication --help
```

If the command is not found, check that its installation directory is on your
`PATH`. In an npm project, use `npx grapharch` instead of `grapharch`.

## Publish your project

Choose the guide for the publication you want:

| Publication | Guide |
| --- | --- |
| Related ontologies with linked vocabulary chapters | [Ontology families](ontology-families.md) |
| A standards-style specification and PDF | [Specification renderer](specification-renderer.md) |
| A documentation website of guides and topics | [Self-hosted documentation](self-hosting.md) |
| Separate specification, companion website, and PDF | [Publication families](publication-families.md) |

For an ontology family, define your project and target in `grapharch.ttl`, then
read the existing repository checkout directly:

```sh
grapharch publication build-family \
  --local-repository . \
  --definitions grapharch.ttl \
  --project urn:example:project \
  --target urn:example:specification \
  --output-root output/docs
```

Replace the example identifiers with those in your configuration. The
[ontology-family guide](ontology-families.md) explains membership, namespace
hints, remote repositories, and the generated files. Source files stay in your
project; generated files go under the chosen output directory.

Use the same pinned GraphArch version and publication command in CI. Publish the
generated files to GitHub Pages or another static host. Browse the
[showcase gallery](../showcases.md) for working output before choosing your
publication style.

## Build from source

Build from source when contributing or testing unreleased changes. Normal
installation uses the prebuilt channels above.

Install the [development prerequisites](../other/contribute/rust-install.md),
then run:

```sh
git clone https://github.com/EKGF/grapharch.git
cd grapharch
cargo build --release --locked
./target/release/grapharch --help
```

On Windows, the executable is `target\release\grapharch.exe`. To generate this
repository's documentation with the built executable:

```sh
./target/release/grapharch publication build-site \
  --source docs \
  --definitions docs/grapharch.ttl \
  --project urn:grapharch:docs \
  --target urn:grapharch:docs:website \
  --output-root output/docs
```

Open `output/docs/site/index.html`, or serve that directory with your preferred
local web server. The [examples guide](examples.md) contains contributor commands
for running individual fixtures and their tests.

For separate collection, planning, rendering, and inspection commands, see the
[publication pipeline guide](publication-pipeline.md). Check the
[status page](../other/status.md) before relying on a planned capability.
