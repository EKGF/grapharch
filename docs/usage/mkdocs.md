---
title: Import MkDocs sites
parent: Usage
nav_order: 23
permalink: /usage/mkdocs/
---
# Import MkDocs sites

MkDocs import is available in source builds from `main`. Check the release notes
before using these commands with a prebuilt binary.

Point GraphArch at an existing MkDocs repository:

```bash
grapharch build /path/to/repository --output output/imported-docs
```

When the repository contains `mkdocs.yml` or `mkdocs.yaml`, GraphArch imports its
instructions and collects the original Markdown and assets. It writes
`output/imported-docs/site/index.html` and `output/imported-docs/documentation.nq`.
The exported graph contains the selected sources, configuration, provenance,
and publication plan. Rendering that graph does not need MkDocs, Python, the
original repository, or its plugins.

An explicit `grapharch.ttl`, `project.ttl`, or `.grapharch/project.ttl` takes precedence. Invalid explicit
configuration is an error; GraphArch does not silently switch to the MkDocs
importer. Having both MkDocs configuration filenames is also an error.

## What is translated

| MkDocs source | GraphArch interpretation |
|---|---|
| `site_name`, `site_description` | Project title and default page description. A page's own description takes precedence. |
| `site_url` | Canonical publication URL, sitemap, and social URL metadata. Local links remain relative and work below a deployment prefix. |
| `repo_url` | Source repository identity and repository link. |
| `docs_dir` | Bounded content root, defaulting to `docs`, confined to the repository. |
| `nav` | Ordered, nested, independent navigation entries; repeated placements of a page, custom labels, external HTTP(S)/email links, and page heading anchors are preserved. |
| No `nav` | Directory navigation, including supported `.pages` hints, without executing the awesome-pages plugin. `title`, `hide`, `nav`, and `collapse: false` are accepted; unknown options fail. |
| YAML frontmatter or native leading `title: ...` metadata | Page metadata, kept separate from the Markdown body. |
| `pymdownx.snippets.base_path` | Explicit local include roots, confined to the repository. |
| `pymdownx.snippets.auto_append` | Shared abbreviation dictionaries, retained as source-derived SKOS concepts. General-purpose automatic Markdown appenders are not supported. |
| Markdown, figures, local images, tabs, callouts, footnotes | Existing shared content collectors and HTML/PDF projections. |

The importer uses GraphArch's documentation layout. A Material, MkDocs, or
Bootswatch theme name produces an explicit presentation-substitution warning;
it does not cause GraphArch to execute or copy that theme. Theme options that
GraphArch does not project are reported and retained in the source configuration.
Custom Jinja directories, CSS, and JavaScript require a source migration or
native GraphArch theme/renderer support. Supplying explicit GraphArch
configuration does not make those programs executable.

## Supported extensions and plugins

The importer recognizes `abbr`, `admonition`, `meta`, `toc`, `tables`,
`footnotes`, `fenced_code`, `codehilite`, `attr_list`, `md_in_html`,
`pymdownx.details`, `pymdownx.tabbed`, `pymdownx.superfences`,
`pymdownx.highlight`, `pymdownx.tasklist`, and `pymdownx.snippets`.
These names select capabilities already implemented by GraphArch. They do not
load the Python extension. Support for `attr_list` and `md_in_html` is limited
to GraphArch's documented semantic constructs; arbitrary HTML or attributes
remain errors.

Recognized plugins are `search`, `awesome-pages`, and `include-markdown`.
GraphArch supplies its own local search, directory-navigation collector, and
bounded include collector. The supported option set is deliberately explicit:
unknown plugin or extension options fail with their configuration path.
For example, arbitrary `custom_fences`, custom search-language processing,
and Python API generation through `mkdocstrings` are not silently approximated.

For shared abbreviations and snippets:

```yaml
markdown_extensions:
  - abbr
  - pymdownx.snippets:
      base_path: [., docs]
      check_paths: true
      auto_append: [abbreviations.md]
```

`abbreviations.md` must contain standard `*[TERM]: Meaning` declarations.
Duplicate or conflicting terms, missing include files, ambiguous include roots,
cycles, traversal, and unsafe source assets fail before publication.

## Unsupported behavior is reported

Python hooks, YAML executable tags, environment tags, YAML aliases, inherited
configuration, unknown plugins, custom executable Markdown extensions, and
unrecognized top-level settings are rejected. Nothing is executed while
interpreting the configuration. Error messages identify the setting so the
author can remove it or express the intended behavior in source-owned
GraphArch configuration.

Ordinary project metadata and MkDocs build/development settings that do not map
to a GraphArch presentation are retained with an explicit warning. The original
configuration and its content digest are kept in the graph; successful import
warnings are also graph records. A successful import is not a claim of arbitrary
MkDocs plugin compatibility or pixel-identical theme reproduction.
