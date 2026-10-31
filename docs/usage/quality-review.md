---
title: Quality review
parent: Usage
nav_order: 12
permalink: /usage/quality-review/
grapharch:
  subject: urn:grapharch:docs:topic:quality-review
---

# Review source quality

GraphArch preserves RDF statements even when sources describe the same resource
differently. Multiple labels or definitions are not automatically RDF errors.
They can represent alternative names, editorial disagreements, or different
perspectives that deserve review.

Publication planning identifies distinct values for the same resource,
annotation predicate, language tag, and datatype. It checks labels, preferred
labels, titles, definitions, descriptions, and Markdown descriptions. A French
translation alongside an English label does not trigger this finding. Repeated
identical statements from several sources do not trigger it either.

A finding retains every competing value and its source. Git sources identify
the exact commit and file. The KG stores these as warning-level `ga:Diagnostic`
resources, with diagnostic fragments, the affected resource, source provenance,
and the observed revision. Existing documentor warnings, including missing
labels and definitions, are retained with their immutable documentation
contribution and included in the same report.

## What reviewers see

An affected Markdown resource description receives a warning callout containing
the competing statements. Specifications and PDFs include **Quality issues** as
a separate numbered appendix. If Acknowledgements is appendix A, the quality
report becomes appendix B and has its own entry in the contents.

The report groups findings by warning type, such as **Missing English source
definition**, and lists the affected resources underneath each heading. For
competing annotations, each resource also retains the alternative statements
and their sources. This lets reviewers address one kind of issue across the
publication without reading the same warning repeatedly.

Companion websites provide a separate report linked from their footers.
Paginated specifications link to the report on their overview page. Rich
descriptions and different versions of the truth are all retained; GraphArch
does not choose one as the truth.

A publication renders the findings pinned to its plan. Exporting the KG,
removing the source files, and rendering the restored graph retains the report.
A later source edit cannot change an earlier publication's findings.

## Use the report in CI

Warnings appear in the publication manifest as well as the KG. Warning-only
publications succeed and are recorded as `published-with-warnings`.
`--fail-fast` and `--keep-going` continue to govern actual execution errors;
unsupported markup, malformed input, and missing required release content
remain errors.

Teams can review the report on each CI publication and resolve disagreements in
the source repositories. If several perspectives are intentional, the report
keeps them visible. Consumers that enforce stricter editorial rules can inspect
the manifest's diagnostic codes and severities in their own CI policy.

Reports are bounded to 1,024 findings per target and 128 value/source records
per competing-annotation finding. Exceeding a budget fails explicitly instead
of publishing an incomplete report.
