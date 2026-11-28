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

Material for MkDocs configurations import unchanged. Each plugin is read as
data, mapped to a GraphArch capability, or reported with a typed warning; a
plugin never fails the import and is never executed:

| Plugin | GraphArch behavior |
| --- | --- |
| `search` | Built-in local search; `lang` and other options are warnings |
| `awesome-pages` | Directory navigation from `.pages` and `.pages.yaml` |
| `include-markdown` | Bounded include collector |
| `exclude` | Each `glob` becomes a `ga:excludeSourcePattern`; `regex` is a warning |
| `build_plantuml` | One `ga:PlantUmlFileCollector` use per `.puml` file under `diagram_root`/`input_folder`, written to the plugin's `output_folder`; rendering is local, no server is contacted |
| `material-ekgf` | The EKGF theme; plugin options are warnings |
| `git-revision-date-localized` | Git provenance; display options are warnings |
| `exclude-search`, `kroki`, `gen-files`, `redirects`, `social`, `tags`, unknown plugins | Warnings (`mkdocs-native-equivalent`, `mkdocs-not-executed`, `mkdocs-presentation-substitution`) |

Material's Python YAML tags are read as data, never imported:
`!!python/name:material.extensions.emoji.to_svg`,
`!!python/name:material.extensions.emoji.twemoji`,
`!!python/name:pymdownx.superfences.fence_code_format` (Mermaid custom
fences) and `!!python/object/apply:pymdownx.slugs.slugify` with its `kwds`.
Any other tag drops its option with an `mkdocs-unknown-yaml-tag` warning.
`extra`, `extra_css`, `extra_javascript`, `hooks` and `theme.custom_dir` are
never injected, rendered or executed; each entry is a warning that names the
native equivalent where one exists.

Markdown extensions are an explicit list. Unknown extension options still fail
with their configuration path, because an extension changes the meaning of the
Markdown it reads.

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
A term declared again keeps its later meaning, as in MkDocs, and the
redefinition is recorded as an `abbreviation-redefined` warning. Missing
include files, ambiguous include roots,
cycles, traversal, and unsafe source assets fail before publication.

## Unsupported behavior is reported

YAML aliases, inherited configuration, custom executable Markdown extensions,
and unrecognized top-level settings are rejected. Nothing is executed while
interpreting the configuration. Error messages identify the setting so the
author can remove it or express the intended behavior in source-owned
GraphArch configuration.

Ordinary project metadata and MkDocs build/development settings that do not map
to a GraphArch presentation are retained with an explicit warning. The original
configuration and its content digest are kept in the graph; successful import
warnings are also graph records. A successful import is not a claim of arbitrary
MkDocs plugin compatibility or pixel-identical theme reproduction.
