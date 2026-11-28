---
title: Import mdBook books
parent: Usage
nav_order: 24
permalink: /usage/mdbook/
---
# Import mdBook books

mdBook import is available in source builds from `main`. Check the release notes
before using it with a prebuilt binary.

Point GraphArch at the directory that holds `book.toml`:

```bash
grapharch build /path/to/repository/book --output output/book
```

GraphArch reads `book.toml` and `SUMMARY.md` as data. It never runs mdBook,
its preprocessors, its renderers, or any script the book configures. An
explicit `grapharch.ttl`, `project.ttl`, `.grapharch/project.ttl` or MkDocs
configuration in the same directory takes precedence.

## What is translated

| mdBook source | GraphArch interpretation |
|---|---|
| `[book] title`, `description` | Project title and default page description. |
| `[book] src` | Bounded content root, defaulting to `src`, confined to the book. |
| `[output.html] git-repository-url` | Source repository identity and repository link. |
| `SUMMARY.md` | Ordered, nested navigation: prefix and suffix chapters, part titles as sections, numbered chapters with their nesting. `SUMMARY.md` itself is not published as a page. |
| Draft chapters (`[Title]()`) | Omitted: they have no page yet. |
| `README.md`, else the first chapter | The home page, recorded as `ga:homeSourcePath`. The chapter keeps its own route as a redirect. |

Chapter destinations must be Markdown files relative to the content root. A
URL, an absolute path, a path leaving the book, or a missing file is an error.

## Settings that are not executed

These settings remain in the retained source and each one produces a
warning diagnostic in `documentation.nq`:

| Setting | Diagnostic code |
|---|---|
| `[preprocessor.<name>]` | `mdbook-preprocessor-not-executed` |
| `[output.<name>]` other than `html` | `mdbook-output-not-produced` |
| Other `[output.html]` options, such as `additional-css`, `additional-js`, `mathjax-support` or `fold` | `mdbook-presentation-substitution` |
