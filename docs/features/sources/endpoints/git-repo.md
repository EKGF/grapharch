---
title: Git repositories
parent: Collect existing sources
nav_order: 3
permalink: /features/sources/endpoints/git-repo/
grapharch:
  subject: urn:grapharch:docs:topic:features:sources:endpoints:git-repo
---

# Read a repository without copying it into your documentation project

An ontology-family build can read an existing local checkout or a selected
revision of a remote Git repository. Source content remains in its owning
repository; generated publications go into the chosen output directory.

## Local checkout or remote revision

- **Local checkout:** read the existing working tree, including uncommitted
  changes, without changing it.
- **Remote repository:** select a branch, tag, or commit, resolve it once, and
  collect files from that exact revision.

Remote builds use a reusable cache and incremental fetching. `--git-cache`
selects a persistent cache location when needed. A failed fetch or missing ref
is an error; a cached older revision is not silently substituted.

Independent RDF files are collected separately. Repository-owned publication
hints can identify the family, its publisher, and the requested output, avoiding
a second copy of that configuration in the tool's repository. Configure
submodules as separate sources.

## Keep the source revision visible

The KG retains commit identity, author and committer attribution, timestamps,
and immutable source-file locations. A remote build records the selected
commit. Local working-tree provenance identifies HEAD as its basis while content
hashes identify the files actually read; uncommitted content is not described as
identical to HEAD.

See the [ontology-family source commands](../../../usage/ontology-families.md)
for local and remote configuration and the
[publication workflow](../../workflow.md) for CI use.
