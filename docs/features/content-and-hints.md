---
title: Reusable content and hints
parent: Features
nav_order: 2
permalink: /features/content-and-hints/
grapharch:
  subject: urn:grapharch:docs:topic:features:content-and-hints
---

# Reuse content without fixing its presentation

A resource's explanation should not depend on one filename or one website
layout. GraphArch gives documented subjects and fragments stable identities.
An authored explanation can be linked to a generated vocabulary entry and reused
in several publications.

## Connect prose and structured resources

Markdown provides the narrative: introductions, examples, guides, and editorial
explanations. RDF supplies resource identities, labels, definitions,
relationships, and constraints. Generated fragments make those statements
readable while keeping links to their subjects and source material.

Rich RDF descriptions can use the EKGF `markdownDescription` annotation. The
supported Markdown constructs are the same ones used in authored documentation.
GraphArch retains the original statements; it does not invent missing
explanations or silently fetch ontology imports to complete them.

## Supply publication hints at the source

Website frontmatter can name a topic, assign a parent, set its navigation order,
and choose its URL. For example:

```yaml
---
title: Publishing a data product
parent: Guides
nav_order: 2
permalink: /guides/publishing/
grapharch:
  subject: urn:example:documentation:publishing
---
```

The stable subject lets a source move without changing its logical identity.
RDF publication hints can configure headings, reference categories, publisher
identity, module book boundaries, and publication targets. Keep those hints in
the ontology repository or in a separate project definition as appropriate.

Hints have defined meanings and are validated. Unrecognised instructions do not
become arbitrary executable behaviour. The
[GraphArch ontology](../other/ontology/README.md) documents the vocabulary.

## Arrange each publication for its readers

A publication target selects a scope and an arrangement. A specification can
start with conformance and vocabulary definitions; a companion website can
start with examples and guides. They can deliberately reuse an explanation
without turning every companion article into normative specification content.

See [specification, companion site, and PDF](../usage/publication-families.md)
for the model and a complete example.
