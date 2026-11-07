---
title: Content groups and callouts
layout: default
parent: Usage
permalink: /usage/content-groups/
grapharch:
  subject: urn:grapharch:docs:topic:usage:content-groups
---
# Content groups and callouts

GraphArch's Markdown collector recognizes Material-style content tabs (`===`),
admonitions (`!!!`), and collapsible admonitions (`???`, or `???+` initially
expanded). Headers have a quoted title; admonition titles are optional. Bodies
use four-space indentation and can contain ordinary Markdown or nested groups.
Fenced and indented code examples are preserved literally.

```text
=== "Business audience"

    An explanation for business readers.

    !!! tip "Start small"

        Choose one use case.

=== "Technical audience"

    ??? note "Implementation detail"

        Detail that can be expanded on the website.
```

Collection normalizes these constructs into a typed content-boundary model
stored in the KG Markdown fragment. The model describes alternatives,
alternative titles, admonition categories, and disclosure state. Ordinary
Markdown remains between the boundaries, so links, images, headings, and
provenance use the normal collection pipeline. Rendering never rereads the
source file or requires MkDocs.

HTML presents alternatives as keyboard-accessible tabs, with arrow, Home, and
End navigation. Without JavaScript, every alternative remains visible.
Collapsible admonitions use native details/summary controls. PDF presents every
selected alternative in order under its title, and expands all selected
admonition content. `inline` and `end` source presentation hints are accepted;
they do not impose floating text on the output.

The retained boundary format is a `grapharch-content` fenced JSON record with a
`kind`: `alternatives`, `alternative`, `admonition`, or `end`. Titles and category
names are data, never executable HTML or CSS. Records are validated and nested
boundaries must balance. This format is collector output, not required authoring
syntax.

Selecting a heading inside a group preserves its enclosing context through
`ga:contentPrefix` and `ga:contentSuffix`. The selected fragment body stays an
unchanged slice of the normalized document. Independent rendering applies that
context; whole-page reconstruction joins the original bodies without splitting
one original group into several groups.

Limits are explicit: 1 MiB source Markdown, 256 containers, 16 nesting levels,
512-byte titles, and 64-byte category names. Invalid titles, unknown header
options, missing indented bodies, malformed boundary records, and unbalanced
containers fail rather than silently appearing as ordinary text.
