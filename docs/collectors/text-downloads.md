---
title: Text Download Collector
parent: Collectors
nav_order: 10
permalink: /collectors/text-downloads/
grapharch:
  subject: urn:grapharch:docs:collectors:text-downloads
---

# Text Download Collector

The Text Download Collector retains UTF-8 files as website downloads. Their
routes follow their paths beneath the documentation source root.

## What it documents

Downloads accompany [pages and guides](../documentable-things/authored-documentation.md)
or [published specifications](../documentable-things/published-specifications.md).
Examples include a Turtle vocabulary and a plain-text licence. These files are
published for readers to download rather than converted into additional pages.

## Include downloadable files

Keep the files beneath the documentation root:

```text
docs/
  index.md
  downloads/
    terms.ttl
    LICENSE.txt
```

Website discovery retains the `.ttl` and `.txt` files at `downloads/terms.ttl`
and `downloads/LICENSE.txt`. A page can link to them:

```markdown
[Download the vocabulary](downloads/terms.ttl)
[Read the licence](downloads/LICENSE.txt)
```

See the [website workflow](../usage/self-hosting.md) for the complete build
command. To generate reference entries from the RDF inside a file, configure
the [RDF File Collector](rdf-files.md); retaining it as a download does not run
the RDF documentation generators over its contents.
