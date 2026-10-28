# Distribution maintenance

GraphArch distributes one CLI through standalone GitHub release ZIPs, platform-specific Python wheels, and npm packages. The executable in each platform wheel is copied byte-for-byte into its ZIP and npm native package. Installation smoke checks run on every supported native runner before release assembly. Linux requires glibc 2.28 or newer; macOS requires 11 or newer; Windows uses the static runtime. x64 and ARM64 are supported. Other platforms fail explicitly.

No registries have been published by this implementation. PyPI `grapharch`, npm `grapharch` and the `@grapharch` organization scope, and crates.io `grapharch` require verified publisher ownership before enabling package publication. Do not add success/version badges before these exist.

## Validate a candidate

Run the **Build and distribute** workflow manually with both publishing switches disabled. This builds six platforms, audits runtime dependencies, runs offline installs through Python/npm and the standalone ZIP, and uploads `release-distribution`. Inspect that artifact and workflow results before publishing.

Source verification and native jobs save compiler caches only after success.
GitHub cache entries are immutable: caching a canceled or failed build can leave
a partial entry that later successful builds cannot replace. Remove such an
entry before expecting a complete dependency cache to be saved under that key.

The `publish` switch creates the GitHub release. The separate `publish-packages` switch enables registry publication and the Homebrew tap update. A native-only release can therefore be published before registry ownership is ready. Tag pushes build and publish native GitHub artifacts, while registry publishing remains disabled. Only stable `MAJOR.MINOR.PATCH` versions are supported. Bump Cargo.toml and run `python3 packaging/distribute.py sync-version`; the reviewed version and tag must agree. Version bump hooks do not push changes or tags.

Configure protected GitHub environments `release`, `pypi`, `npm`, `crates-io`, and `homebrew`. Register trusted publishers for this repository's `distribute.yml` workflow and corresponding environment in PyPI, every npm package, and crates.io. npm publishing requires permission to the product scope and main package. The Homebrew environment needs `HOMEBREW_TAP_TOKEN` with narrowly scoped write access to `legra-ai/homebrew-tap`; the generated formula uses verified release checksums. No sibling checkout is modified by builds.

`cargo binstall grapharch` requires the initial crates.io publication so that binstall can discover the manifest. Metadata selects the GitHub ZIP and disables quick-install and compilation strategies. Before the first published version, binstall correctly reports missing artifacts rather than compiling. Source crate packaging includes embedded ontology definitions and renderer/font assets, while excluding examples and generated outputs. Archive/package notices include both the default document fonts and bundled Liberation fonts.

## Consumer GitHub Action

Use one action step to install a pinned release and build an ontology family. Check out the consuming repository first, then pin the action itself to a reviewed commit SHA:

```yaml
- uses: actions/checkout@v4
- uses: EKGF/grapharch@<reviewed-commit-SHA>
  id: docs
  with:
    version: '0.1.0' # must already be published as a GitHub release
    source-kind: checkout
    source: .
    output: build/docs
```

The default `mode` is `build-family`. `source-kind` defaults to `directory`; choose `checkout` explicitly to collect a local Git repository with provenance. GraphArch discovers the project and target from publication hints in the source RDF. To use external publication definitions instead, supply `definitions`, `project` and `target` together. The output root defaults to `build/docs`. Paths with spaces are supported; all CLI arguments are passed directly without shell interpretation. The build requires a release supporting `build-family --json` report schema version 1.

Outputs `publication-path`, `pdf-path` and `kg-path` are canonical absolute paths validated to exist beneath the chosen output root. The publication directory contains `index.html`; the KG is checked separately. `pdf-path` is empty when the renderer does not produce a single collection PDF. Upload `publication-path` as the website, and optionally include the separate `kg-path` archive in deployment packaging. The action does not deploy or reread sources to assemble outputs.

Use `mode: install` for setup alone, then invoke any CLI command yourself. `version` is required and exact, without `v`; `cache` defaults to `true`. Outputs `version` and the absolute `binary-path` are available in both modes. The verified installation directory is added to PATH. A checksum is fetched and checked even on cache hits. Unsupported platforms, missing artifacts, corrupt archives and version mismatches fail without compilation or alternate downloads. Invalid inputs fail before downloading; CLI failures preserve their exit status. No registry credentials are needed.
