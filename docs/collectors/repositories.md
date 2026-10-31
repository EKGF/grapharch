---
title: Repository Scanners
parent: Collectors
nav_order: 7
permalink: /collectors/repositories/
grapharch:
  subject: urn:grapharch:docs:collectors:repositories
---

# Repository Scanners

Repository discovery locates supported files and dispatches specialised
collectors. It is transport and provenance, not a single collector that
understands every file or every RDF resource kind.

## What it documents

Family builds discover RDF files supporting [ontologies, families and modules](../documentable-things/ontology-families.md),
[SHACL](../documentable-things/shacl-shapes.md),
[SKOS](../usage/skos-taxonomies.md), and [EKGF use cases](../usage/use-case-trees.md).
RDF types and explicit membership determine recognition. A folder named
`taxonomies` or `modules` does not establish semantic membership.

## Choose local edits or a remote revision

For an existing local checkout with source-owned publication hints:

```sh
grapharch publication build-family --local-repository ../my-ontologies \
  --output-root output/local-family
```

This reads the working tree, including uncommitted edits. The documentation KG
records that state; it does not claim those edits equal a committed revision.

For a remote GitHub repository:

```sh
grapharch publication build-family --github example/my-ontologies \
  --git-ref develop --output-root output/remote-family
```

Replace the repository with your own. A branch tracks its latest fetched commit
on each build; a full commit ID pins an immutable input. `--git-repository URL`
also accepts other Git hosts. Without `--git-ref`, the remote default branch is
used. A cached bare repository avoids repeated full clones; jobs read the one
resolved commit, not a changing checkout.

The source must supply unambiguous project/target hints, or the command needs
explicit definitions, project, and target overrides. See the [family workflow](../usage/ontology-families.md).
Family discovery does not automatically import every Markdown, image, code file,
or HTML template: those require the appropriate workflow and specialised jobs.
Configured Git Markdown, code, image, and legal collectors read pinned blobs with
commit and blob provenance. See [Markdown](markdown-files.md) and
[assets and code](assets-and-code.md) for those separate paths.
