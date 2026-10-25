---
title: Git Repository
layout: default
parent: Endpoint
nav_order: 3
grapharch:
  subject: urn:grapharch:docs:topic:features:sources:endpoints:git-repo
permalink: /features/sources/endpoints/git-repo/
---

# Source — Git repository

Use a local checkout or a remote Git repository as the source for an ontology
family publication:

- **Local working tree:** read an existing clone directly, including uncommitted
  edits, without copying or changing the checkout.
- **Remote Git repository:** fetch an explicit branch, tag ref, or commit into a
  reusable bare cache, then read RDF blobs directly without a checkout.

The cache lives under the system temporary directory in `grapharch-git-cache`.
`--git-cache` can select a persistent location. Entries are keyed by repository
URL and locked during fetch; subsequent runs fetch incrementally. A build pins
one commit before dispatching its independent RDF file collectors. Network,
missing-ref, parsing, and metadata errors fail the build; cached state is never
silently substituted for a failed fetch.

See [ontology family source commands](../../../usage/ontology-families.md).

## Git provenance

The KG retains the observed commit identifier, separate author and committer
identities, and their timestamps using PROV, Dublin Core, FOAF, and Schema.org.
Each remote RDF file records its commit and immutable source location. Local
working-tree provenance records HEAD as its basis, while collected file hashes
identify actual input contents. HEAD is not presented as the exact content of
uncommitted files.

Current provenance describes the selected commit. Per-file last-change history,
blame, contributor statistics, and automatic release notes are future consumers
of the same cached Git object database.

Supported RDF formats are Turtle, RDF/XML, N-Triples, N-Quads, TriG, and N3.
Configure submodules as separate repository sources.
