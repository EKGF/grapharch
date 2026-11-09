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

Install GraphArch and point it at your content. Start without configuration;
add publication hints when you need to control the result.

Standalone binaries are available from GitHub Releases. Package-manager channels
remain subject to their separate publication status below.

## Install a prebuilt binary

Download the archive for your platform from
[GitHub Releases](https://github.com/EKGF/grapharch/releases). Each archive has an
adjacent SHA-256 checksum file. Verify the download, extract it, and put
`grapharch` (or `grapharch.exe` on Windows) in a directory on your `PATH`.

The release platforms are:

| System | CPU | Requirement |
| --- | --- | --- |
| Linux | x64 or ARM64 | glibc 2.28 or newer |
| macOS | Intel or Apple silicon | macOS 11 or newer |
| Windows | x64 or ARM64 | Native Windows executable |

Musl-based Linux distributions and 32-bit systems are not included in this
release matrix. Select the archive for the system where GraphArch will run.

### Install with cargo-binstall

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) installed:

```sh
cargo binstall grapharch --version 0.2.4
```

This downloads the prebuilt executable for your system. Unsupported platforms
or missing release files fail installation.

### Other installation channels

Python and npm publication is still pending. Homebrew requires the separate tap
publication. These channels install the same CLI once available:

| Channel | Install or run |
| --- | --- |
| Homebrew | `brew install legra-ai/tap/grapharch` |
| Python with uv | `uv tool install grapharch`, or `uvx grapharch --help` for an isolated run |
| Python with pip | `python -m pip install grapharch` |
| npm project dependency | `npm install --save-dev grapharch`, then `npx grapharch --help` |

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

## Start without configuration

For OWL class diagrams, install Java and PlantUML on `PATH` first. Smetana, the
layout engine used by GraphArch, is included in PlantUML; Graphviz is not required.
The GraphArch GitHub Action installs a checksum-verified renderer during setup.
Markdown-only builds and rendering a retained documentation graph do not invoke
PlantUML. See [OWL class diagrams](../features/owl-class-diagrams.md).

```sh
grapharch build ./ontology.ttl
grapharch build ./my-project --output output/preview
# From a directory containing Markdown or RDF:
grapharch build
```

Open `output/grapharch/site/index.html` (or `output/preview/site/index.html`
with the override). The output also contains `documentation.nq`: source content,
provenance, inferred publication definitions, and the resolved plan. HTML is
rendered from that graph, so it can be restored without the source files.

Supported inputs are Markdown and RDF in `.ttl`, `.rdf` (RDF/XML), `.nt`, `.nq`,
`.trig`, and `.n3` files. Local images are collected as assets. No frontmatter,
GraphArch vocabulary, or extra ontology annotations are needed. Markdown titles
come from frontmatter when present, otherwise the first heading or filename.
A single ontology's label supplies the site title; otherwise GraphArch uses the
home page title or directory name. RDF definitions and relationships come from
the supplied graph; missing descriptions are not invented. Shared subjects are
documented once, using statements from all collected RDF files.

The default website uses neutral styling and navigable pages. `README.md` or
`index.md` is the home page; an RDF reference or generated contents page supplies
the home page when neither exists. Git provenance is collected for checkouts.
No publishing authority, approval status, release date, or license is assumed.

Discovery is deterministic and limited to 256 source files and 4,096 directory
entries. Hidden directories, underscore-prefixed entries, `node_modules`,
`target`, `dist`, `build`, `output`, `vendor`, `venv`, and the selected output
directory are excluded. Symlinks are rejected. Large repositories should select
a content subdirectory or configure a scoped publication; the quick RDF reference
is limited to 20,000 named typed subjects.

For an existing MkDocs repository, the same command reads `mkdocs.yml` or
`mkdocs.yaml` and translates supported metadata, navigation, and content settings.
See [Import MkDocs sites](mkdocs.md) for supported constructs, explicit diagnostics,
and the distinction between content import and theme reproduction.

A root `grapharch.ttl`, `project.ttl`, or `.grapharch/project.ttl` is treated as explicit configuration:
a single configured target uses the existing configured publication pipeline.
Invalid configuration remains an error. For multiple targets or another
configuration filename, use the explicit publication commands below.

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
