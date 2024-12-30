---
title: "Ontology"
layout: default
nav_order: 3
---

# Ontology

> [!NOTE]
>
> This is a work in progress.
> At the moment this is all extremely basic and not yet used, just a placeholder.

The idea is to have source handling components that scan a given data source
for documentable items, such as a component that understands OWL Ontologies,
where that source handling component generates documentation objects such as
"chapter", "section" etc, according to an "documentation ontology", into a
temporary triple-store. From there, output-handling components like a PDF
generator or a website generator will create the most appropriate output.

This would allow for being able to generate consistent documentation as both
a website and a document, even though those two things are usually completely
different in structure.

- [documentation.ttl](./documentation.ttl)
- [doc-component-categories.ttl](./doc-component-categories.ttl)
