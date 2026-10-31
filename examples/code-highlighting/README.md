# Multilingual code examples

This small authored specification demonstrates Python, Bash, JavaScript,
TypeScript, JSON, JSON-LD and SPARQL fences. Each block carries the optional
`example` marker for an automatic numbered caption. No code is executed.

From the repository root:

```bash
cargo test --test code_examples multilingual_showcase -- --nocapture
```

To keep the generated HTML and PDF for local review:

```bash
GRAPHARCH_CODE_PREVIEW=output/code-highlighting-review cargo test --test code_examples multilingual_showcase -- --nocapture
```

Open `output/code-highlighting-review/specification/index.html` and its companion
`document.pdf`. The acceptance test exports the documentation KG, removes the
original source directory, restores the KG into a new store, and renders from the
pinned plan. The full showcase gallery also includes this publication.

Aliases and Markdown examples are documented in
[Code examples and syntax colours](../../docs/usage/code-highlighting.md).
