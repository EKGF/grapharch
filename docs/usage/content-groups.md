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

## Included source fragments

Local Markdown collectors accept `--8<-- "path.md"` and
`{% include-markdown "path.md" start="<!--start-->" end="<!--end-->" %}`.
Markers are optional, excluded from the selected text, and must occur exactly
once when supplied. Nested includes preserve indentation and rebase relative
inline links and images to the including page. Reference-style relative links
inside includes currently require conversion to inline link syntax.

Paths resolve against the documentation content root and explicitly configured
include roots. They never resolve against the process working directory.
`MarkdownFile::with_include_roots` adds up to eight roots; ambiguous matches,
absolute paths, parent traversal, symlink escapes, cycles, unknown directive
options, and missing markers fail collection. Each included source is limited
to 256 KiB, all dependencies to 1 MiB, nesting to 16 levels, and dependencies
to 128 per page. Supplied Git blobs require a repository-aware include resolver
and cannot use this local-file mechanism.

The collected contribution retains full included source text and its hash,
selected character ranges, and `prov:wasDerivedFrom` links. Updates outside a
selected range still change dependency provenance. Publication reads the
expanded Markdown from the pinned KG, so exporting the KG and deleting source
files does not lose included content. Abbreviation definitions included from
MkDocs sources do not imply support for MkDocs abbreviation expansion.

## Identifier badges

An authored empty `objective-badge` span with an alphanumeric `data-letter`
becomes a semantic inline identifier. A `letter_prefix` frontmatter value adds
the same identifier to the first level-one heading. Identifiers contain 1–16
ASCII letters or digits; they do not change heading anchors or navigation titles.
The optional source class `objective-badge-sm` does not dictate output styling.

The retained Markdown spelling is an empty link to
`urn:grapharch:badge:IDENTIFIER`. This is an identifier annotation, not a
navigational URL. Shared HTML and PDF renderers display the identifier as a
compact label. Raw HTML is not passed through. Ordinary code examples remain
literal; invalid badge markup is handled by the normal unsupported-HTML error.

Material card collections written as `<div class="grid cards" markdown>` around
an ordinary Markdown list are collected as semantic card groups. HTML presents
the list as responsive cards using the shared theme; PDF retains every item in
reading order. This supports the explicit authoring construct, not arbitrary
HTML or source-owned CSS.
