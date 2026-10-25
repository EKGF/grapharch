---
title: "Design Goals"
layout: default
nav_order: 2
permalink: /design-goals/
grapharch:
  subject: urn:grapharch:docs:topic:design-goals
---

# Design Goals

Create a tool that can extract information from any source and generate
professional documentation from it either as a document or a static website.

Primary focus is to document ontologies and knowledge graphs but ultimately
to document "knowledge" in the broadest sense of the word.

- First usable release: generate DPROD specification HTML, reader HTML, and
  Typst/PDF from one validated graph revision, following the
  [delivery specification](../other/architecture/deliverables.md).
- Shared fragments retain identity across target-specific website and document
  arrangements; ingestion never assumes one ontology equals one book.

> [!NOTE]
>
> An additional angle that will be explored is how to use the tool to generate
> content for LLM agents to consume.

## Robustness

**Robustness** is the number one priority:

- Invalid input and failed operations return actionable errors immediately.
- Incomplete contributions never become publishable revisions.
- Failed updates preserve the last valid revision without reporting success.

## Performance

**Performance** includes throughput and measured edit-to-preview latency.
Use small specialized collectors, bounded parallel execution, streaming with
backpressure, and backend-controlled writes. Incremental execution reuses
independent work and prevents superseded renders from replacing current output.

The program should be fast, so that:

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

- [ ] On the output side, different publication targets reuse the same
      documentation graph and logical fragments. Built-in renderers produce
      final publication artifacts directly:

  - [x] [HTML and CSS for static websites](../features/targets/html.md)
  - [x] [Standards-style specification HTML with a paired PDF](../usage/specification-renderer.md)
  - [x] [Typst source and PDF documents](../features/targets/typst.md)
  - [x] RDF dataset export for inspection and reuse.

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
