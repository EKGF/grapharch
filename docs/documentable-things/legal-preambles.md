---
title: Legal preambles
parent: Documentable Things
nav_order: 9
permalink: /documentable-things/legal-preambles/
grapharch:
  subject: urn:grapharch:docs:topic:documentable-things:legal-preambles
---

# Legal preambles

A formal publication may need copyright notices, license terms, disclaimers and
other approved legal text before its technical chapters. GraphArch can collect
that preamble as a separate fragment, retain its wording and source provenance,
and attach it to explicitly selected publication targets.

## Example: a supported disclosure

This fictional notice illustrates the input structure only. For a real
publication, supply the complete wording approved by its publisher.

```html
<details class="preamble-section">
  <summary>Preamble</summary>
  <h2>Demonstration notice</h2>
  <p>This fictional notice demonstrates the layout of a legal preamble.</p>
  <p>It does not state the terms of any actual publication.</p>
</details>
```

The [Legal preamble collector](../collectors/legal-preambles.md) recognises the
unique `details.preamble-section` element in a configured HTML source. Its
`summary` supplies the fragment title; the remaining supported content supplies
the body. Missing or ambiguous disclosures, missing summaries and empty content
are errors. Supported relative links resolve against the supplied source URL.
The collector does not execute surrounding scripts or template expressions.

This is an explicitly configured input, not automatic legal-text extraction from
arbitrary repository files. The collector page explains how integrations supply
the source and target identities.

## Example: DPROD's source preamble

DPROD's `develop` branch contains the real disclosure in
[`respec/template.html`](https://github.com/EKGF/dprod/blob/develop/respec/template.html).
The GraphArch showcase reads that block from the same pinned repository commit
as the other DPROD inputs. It retains the complete supported preamble; it does
not replace the source notices with a generated summary or new legal language.

Open the [generated DPROD specification](https://ekgf.github.io/grapharch/showcases/artifacts/dprod/specification/index.html)
and expand **Preamble**. Compare it with the dedicated legal front matter in the
[generated DPROD PDF](https://ekgf.github.io/grapharch/showcases/artifacts/dprod/pdf/document.pdf).
The website and PDF present the same retained fragment in different layouts.

## From legal fragment to publication

A target's `ga:preamble` link selects its legal fragment. A specification can
present it as an expandable disclosure, keeping lengthy notices accessible near
the publication metadata. The PDF includes the full text on dedicated front-matter
pages, with justified paragraphs and legal-section styling, before the technical
content. The preamble is separate from ordinary numbered chapters.

See [legal preamble publication](../usage/specification-renderer.md#legal-preamble)
for the target relationship. As with other fragments, renderers read the retained
KG content; they do not reopen the original template when generating a
publication from an exported and restored graph.
