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
existing static specification artifacts into one static package, without an
application framework or server functions. The hosting configuration names its
`platform` explicitly, and the platform alone determines the output layout.
There is no default platform.

```yaml
version: 2
platform: github-pages
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
inventory: ../output/website/site/publication-versions.json
version_path: spec
```

Paths to input artifact trees resolve relative to this YAML file. Publication
paths are relative URL paths. Directory aliases copy their original input tree,
so the generated files also work on a plain static web server. The inputs and
their source RDF are never modified.

```sh
grapharch publication package-site --config publication/hosting.yml \
  --output-root output/deployment
```

The command prints the package directory. Its layout depends on the platform:

| `platform` | Package | Mounted files | Routing |
| --- | --- | --- | --- |
| `github-pages` | `output/deployment/static/` | `static/<mount>/` | none; the host serves files |
| `vercel` | `output/deployment/.vercel/output/` | `static/<mount>/` | `config.json` (Build Output API v3) |

For GitHub Pages, upload `static/<mount>/` as the Pages artifact; a project
site's own prefix, such as `/project/`, is provided by the host. With
`--mount /`, upload `static/`. Pages serves directory indexes, trailing-slash
redirects and a root `404.html` itself, but it cannot set response headers or
route requests. A `github-pages` package therefore rejects `file_aliases`, and
inventory versions that are not materialized locally, with an explicit error
naming each one. GraphArch writes no Vercel paths for it.

Declare `platform: vercel` only for a Vercel deployment. Its `config.json`
adds the routes: directory indexes, `file_aliases` with their content types,
redirects for remote inventory versions, and the `404.html` fallback. GraphArch
only writes these files; deploying them is a separate step.

Version 1 configurations, which had no `platform`, are rejected. Add
`platform` and set `version: 2`.

Packaging rejects existing output, path collisions, path traversal, symlinks,
private Git/environment files, and artifacts exceeding the file or byte limits.
It does not silently overwrite a successful deployment package.

The optional version inventory routes every observed remote version, including
unadvertised previews, on platforms with routing. `version_path` configures its
URL namespace; locally materialized version directories always take precedence. The inventory must
contain an RFC 3339 observation timestamp and distinct safe version slugs.
