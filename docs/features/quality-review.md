---
title: Quality review
parent: Features
nav_order: 5
permalink: /features/quality-review/
grapharch:
  subject: urn:grapharch:docs:topic:features:quality-review
---

# Make documentation quality visible

Generating documentation can reveal problems that are difficult to notice in
individual source files. GraphArch preserves the source statements and records
review findings in the KG, so reviewers can see disagreements instead of
receiving an arbitrarily chosen answer.

## Review competing annotations

Different sources can give the same resource different labels, titles, or
descriptions. GraphArch reports distinct values for the same annotation,
language tag, and datatype, with the supplied values and their provenance.
Different language translations are not treated as competing values.

Multiple versions of the truth are not automatically RDF errors. The report
lets a team decide whether a difference is intentional or needs correction.
Source-aware resource documentation also records missing labels and definitions.

## Read the findings where they matter

Affected descriptions can display warning callouts. A **Quality issues**
appendix groups findings by warning type and lists the affected resources.
Specifications and PDFs keep the report in their contents; companion sites
link to a separate report.

The publication manifest exposes the diagnostic codes and severities for CI
policies. Warning-only publications succeed. Malformed input, unsupported
behaviour, and missing required release content remain execution errors.

## Compare review iterations

Correct a source, rebuild, and inspect the next report. Resolved disagreements
disappear from the new plan. An earlier exported KG still reproduces its
original findings, making the review trail specific to a revision.

See [reviewing source quality](../usage/quality-review.md) for the current
checks, report behaviour, and CI policy boundaries.
