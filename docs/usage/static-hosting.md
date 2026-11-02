---
title: Static hosting packages
parent: Usage
nav_order: 16
permalink: /usage/static-hosting/
grapharch:
  subject: urn:grapharch:docs:usage:static-hosting
---
# Static hosting packages

`grapharch publication package-site` assembles generated website pages and
existing static specification artifacts. It emits a Vercel Build Output API
package, without an application framework or server functions.

```yaml
version: 1
mount: /project
site: ../output/website/site
static_trees:
  - source: ../dist
    destination: spec
  - source: ../archive
    destination: spec/archive
directory_aliases:
  - path: spec/develop
    target: spec
  - path: spec/main
    target: spec/archive/1.0
file_aliases:
  - path: spec/contracts/profile/
    target: spec/profile.ttl
    content_type: text/turtle
```

Paths to input artifact trees resolve relative to this YAML file. Publication
paths are relative URL paths. Directory aliases copy their original input tree,
so the generated files also work on a plain static web server. File aliases
provide explicit canonical resource paths and media types in Vercel's routing
configuration. The inputs and their source RDF are never modified.

```sh
grapharch publication package-site --config publication/hosting.yml \
  --output-root output/deployment
```

The result is `output/deployment/.vercel/output/`: `config.json` records routing,
and `static/` contains the mounted publication. Deploy that directory with
Vercel's prebuilt deployment workflow. For hosting under a GitHub Pages project
prefix, `--mount /` places content at the static root; its relative links retain
the host's project prefix.

Packaging rejects existing output, path collisions, path traversal, symlinks,
private Git/environment files, and artifacts exceeding the file or byte limits.
It does not silently overwrite a successful deployment package.
