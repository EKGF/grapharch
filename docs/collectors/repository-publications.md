---
title: Repository Publication Collector
parent: Collectors
nav_order: 11
permalink: /collectors/repository-publications/
grapharch:
  subject: urn:grapharch:docs:collectors:repository-publications
---
# Repository Publication Collector

This collector reads GitHub pull-request metadata and Vercel deployment metadata
into a native RDF publication inventory. A version-listing documentor turns
that inventory into a page; the website renderer retains the same inventory as
JSON for static hosting rules.

It supports [documentation pages](../documentable-things/authored-documentation.md)
that help readers choose a publication version, rather than discovering ontology
classes from repository metadata.

```turtle
<urn:example:project> ga:collector <urn:example:versions> .
<urn:example:versions> a ga:CollectorUse ;
    ga:collectorType ga:RepositoryPublicationsCollector ;
    ga:source "https://github.com/example/project" ;
    ga:subject <urn:example:inventory> ;
    ga:publicationPath "project/spec/" ;
    ga:unlistedBranchPrefix "dependabot/" .
```

See [Publication versions](../usage/publication-versions.md) for the complete
source-owned canonical-version declarations, generated-page configuration,
required credentials and snapshot freshness behavior. API errors fail the build;
rendering a restored KG requires neither credentials nor deployment API access.
