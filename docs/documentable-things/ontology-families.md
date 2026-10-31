---
title: Ontology families and modules
parent: Documentable Things
nav_order: 3
permalink: /documentable-things/ontology-families/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:families
---

# Ontology families and modules

A family publication documents several selected vocabularies together. Each
vocabulary keeps its identity, namespace, terms, and source provenance. Family
membership is declared through the Artifact Governance registry: a family has
references or artifacts that identify its members. GraphArch does not assume
that every RDF file in a repository belongs to one family.

The current family pipeline accepts OWL ontology declarations and standalone
SKOS concept schemes. A source file currently declares at most one selected
ontology or standalone scheme; the
[SKOS guide](../usage/skos-taxonomies.md) explains standalone scheme selection.
Cross-reference links connect the resources documented in the selected family.

## Explicit modules

Collectors recognise `ga:OntologyModule` and the native FIBO Module type:

```text
<https://spec.edmcouncil.org/fibo/ontology/FND/Utilities/AnnotationVocabulary/Module>
```

For explicitly typed modules, `dct:hasPart` supplies membership links to named
ontologies or nested modules. Native assertions remain intact, while the
collector retains normalised module membership for publication planning.
Directories and file names alone do not create modules.

Modules provide another level of reading structure above their ontologies.
They can appear in a family reference, a paginated site, or a bookshelf whose
module books have independent specifications and PDFs. Membership cycles and
references to uncollected members are reported as errors instead of creating a
partial family silently.

## Configure and publish

See [Publish an ontology family](../usage/ontology-families.md) for source
configuration, registry reuse, authority roles, repository sources, and CLI
commands. See [Bookshelves and module books](../usage/bookshelves.md) for the
publication model suited to large modular families.
