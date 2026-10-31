---
title: Document provenance
parent: Usage
nav_order: 15
permalink: /usage/provenance/
grapharch:
  subject: urn:grapharch:docs:usage:provenance
---
# Document provenance

Every generated specification and PDF ends with a **Document provenance** section.
Companion websites link to a provenance page from their footer. Community drafts
and formal submission profiles share this record while keeping their own branding
and publication status.

The record identifies:

- **Generator:** Grapharch and its version.
- **Build operator:** an explicitly recorded person or CI identity, when supplied.
- **Sources:** each collected Git repository and the exact source commit.
- **Source state:** a pinned committed tree, or a local working tree that may contain
  uncommitted changes.
- **Recent committers:** up to five available commits along the first-parent
  history, with committer names, commit dates and full commit identifiers.
- **Git tags:** known tags that point at that exact source commit.
- **Releases:** explicitly recorded releases associated with that commit.

Recent committers are not the document's authors or its full contributor list.
Editors and authors remain publication metadata. A Git tag does not establish
that a GitHub release exists, or that a standards body has approved the document.

## Git sources

[Repository Scanners](../collectors/repositories.md) retain provenance during
collection. Website collection also discovers enclosing Git clones for the
documentation directory and its publication definitions. Plain directories remain
valid sources, while malformed Git metadata stops the build. A remote branch such as `develop` is resolved to one commit for each
build. Fetches reuse the repository cache; the provenance records that build's
resolved commit and tag associations. Moving or deleting a remote tag changes
subsequent provenance without changing already collected revisions.

A local repository records its configured origin when available and separately
retains its local checkout location in the documentation graph. Public provenance
shows that its inputs are working-tree files rather than asserting that every
file matches HEAD. Git credentials and committer email addresses are not printed.

When a source has no collected Git provenance, the report says so. A repository
URL mentioned in ordinary RDF does not become evidence that the document was
built from that repository.

## Record the build operator

Supply generation metadata in the publication definitions when your CI knows
which identity generated the document. For example:

```turtle
@prefix prov: <http://www.w3.org/ns/prov#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

<urn:example:community-pdf> prov:wasGeneratedBy <urn:example:documentation-build> .
<urn:example:documentation-build>
    prov:wasAssociatedWith <urn:example:documentation-ci> .
<urn:example:documentation-ci> rdfs:label "Example documentation CI" .
```

The subject of `prov:wasGeneratedBy` is the publication target. Grapharch does not
infer the build operator from a local username or from the person who last
committed a source file. If no operator was supplied, the report says it was not
recorded.

## Associate a release with a source commit

If your publication definitions know that a release was made from the collected
commit, record the association explicitly. The commit entity is retained by the
Git collector in the exported documentation graph with its full SHA as
`dct:identifier`.

```turtle
@prefix doap: <http://usefulinc.com/ns/doap#> .
@prefix prov: <http://www.w3.org/ns/prov#> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix schema: <https://schema.org/> .

<https://github.com/example/ontology/releases/tag/v1.1>
    a doap:Version ;
    dct:title "Community release 1.1" ;
    prov:wasDerivedFrom <urn:example:collected-commit-entity> ;
    schema:url <https://github.com/example/ontology/releases/tag/v1.1> .
```

Replace the commit entity with the Git collector's actual entity; a release
linked to a different commit is not listed. Grapharch does not fetch release
pages while rendering.

## Render from the documentation graph

All source evidence comes from the pinned documentation graph. Exporting that
graph and rendering it later preserves the source revisions and recent committer
records even when the original clone is no longer on disk. The generator version
identifies the Grapharch executable that performs the later rendering.
