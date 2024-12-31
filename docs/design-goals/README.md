---
title: "Design Goals"
layout: default
nav_order: 2
permalink: /design-goals/
---

# Design Goals

Create a tool that can extract information from any source and generate
professional documentation from it either as a document or a static website.

Primary focus is to document ontologies and knowledge graphs but ultimately
to document "knowledge" in the broadest sense of the word.

- First milestone: Replace the current code that generates the DPROD webpage (<https://ekgf.github.io/dprod/>)

> [!NOTE]
>
> An additional angle that will be explored is how to use the tool to generate
> content for LLM agents to consume.

## Robustness

**Robustness** is the number one priority:

- [ ] The program should never panic.
- [ ] The program should never enter an inconsistent state.
- [ ] The program should never crash.

## Performance

**Performance** is a close second, the program should be fast, so that:

- [ ] It can scan large knowledge graphs or other large amounts of data
      within a reasonable amount of time (for instance within the duration
      of a batch job that can finish in one night).
- [ ] It can process a large number of files or git repositories for
      documentable items in a short amount of time.
- [ ] It will not cause much additional cost for people deciding to
      integrate it in their CI/CD pipelines.

## Scalability

- [ ] The program should use as little memory as possible and be designed
      to leverage streamed processing where appropriate allowing it to scale
      to any size of data.
- [ ] The program should be designed to be easily parallelizable or run things
      concurrently.

## Extensibility

- [ ] The program should be designed to be easily extensible to add new
      features or documentable items. It features a generic documentation model
      that can be constructed by "plugins" that will implement the logic to
      extract information from the data and transform it into documentation
      components such as "chapters", "sections", "tables", "lists", etc in
      the documentation model.

- [ ] On the output side, many different types of documentation can be generated
      from the same documentation model. For instance, a documentation model can
      be exported to a set of markdown files, to a set of html files, to a
      set of json files, etc.

  - [ ] [typst files that are used to generate professional PDFs](../features/targets/typst.md)
  - [ ] json files that can be used for further processing or
        feeding into a search engine.
  - [ ] [markdown files that can be used for further processing by tools
        like MkDocs](../features/targets/markdown-for-mkdocs.md)
  - [ ] [reStructuredText files that can be used for further processing by tools
        like Sphinx](../features/targets/rst-for-sphinx.md)

- [ ] Most of the logic of the program will be published as a library, so that
      it can be reused in other projects.

## Configurability

- [ ] The program should be configurable through configuration files.
- [ ] The program should be configurable through environment variables.
- [ ] The program should be configurable through command line arguments.
- [ ] The program should be configurable through source data annotations.
- [ ] The program should be configurable through code.

## Ease of use

- [ ] The program should be designed to be easy to understand, so that:
  - [ ] People can easily understand how to implement their own plugins if
        they need custom logic.
  - [ ] People can understand what is the output of the program and how to
        use it.
- [ ] Editors should be able to run GraphArch on their own machine and see the
      updated documentation straight away, ideally as a preview option in their
      favorite editor (Microsoft Visual Studio Code for instance).
  - [ ] Easily download and install GraphArch on Windows, MacOS and Linux.
  - [ ] Generate the various types of output in seconds.
  - [ ] VS Code preview.
  - [ ] IntelliJ preview.
