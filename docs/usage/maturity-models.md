---
title: Maturity models
layout: default
parent: Usage
permalink: /usage/maturity-models/
grapharch:
  subject: urn:grapharch:docs:topic:usage:maturity-models
---
# Maturity models

`MaturityModelDocumentor` generates fragments for the EKGF maturity-model
vocabulary's `Model`, `Pillar`, `CapabilityArea`, `Capability`, and `Level`
instances. It reads a pinned collected KG revision. It does not execute the
legacy Python/MkDocs generator or read source files during documentation.

The vocabulary namespace is
`https://raw.githubusercontent.com/EKGF/ontology-maturity-model/main/maturity-model.ttl#`.
The hierarchy comes from `pillarInModel`, `inPillar`, `inArea`, and
`levelInModel`. Parents must exist with the expected explicit type; missing,
ambiguous, and incorrectly typed parents fail. This documentor does not perform
OWL inference. Capability numbers and explicit sort keys determine natural
numeric order, so `A.1.2` precedes `A.1.10`.

Source `rdfs:label` supplies labels, `rdfs:comment` supplies plain-text taglines,
and `dct:description` supplies the Markdown descriptions used by this model's
authoring convention. English or language-neutral annotations are selected;
ambiguous selected values fail. Generated hierarchy links use the original
subject identities so publication plans can choose their routes.

```rust
let documentor = MaturityModelDocumentor::new(dataset, snapshot, project)?;
documentor.document(&writer).await?;
let subjects = documentor.subjects().await?;
// Bind documentor.fragment_id(&subject) to components in the target profile.
```

This is a library documentor. Publication orchestration must invoke it and bind
its fragments to the desired components; RDF collection alone does not invoke
it automatically. Each fragment retains its input revision and contribution.
Changes regenerate the affected fragment content; removed subjects retire the
corresponding generated contributions while historical snapshots remain valid.

The current source vocabulary does not define assessment statements linking
capabilities to levels. Existing `levels.md`, introductory, dimensions, value,
and approach fragments remain authored content to collect and place alongside
these generated fragments. The documentor does not invent assessment claims or
copy that prose into another source file.

Execution has explicit limits: 4,096 model subjects, 8 MiB of hierarchy metadata,
256 KiB per annotation, and 512 KiB per generated fragment. Descriptions are
read one subject at a time. Exceeding a limit fails with an explicit error.
