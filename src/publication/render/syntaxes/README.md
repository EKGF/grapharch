The SPARQL grammar is vendored unchanged from [LinkedData 1.0.4](https://github.com/blake-regalia/linked-data.syntaxes/releases/tag/v1.0.4),
`assets/sparql.sublime-syntax` in `LinkedData.sublime-package` (18 November 2021).
It covers SPARQL 1.1 and SPARQL-star. Its ISC license is retained in
`packaging/licenses/linked-data-syntaxes.txt`.

`build.rs` compiles it with Syntect into an embedded syntax set. Publishing never
downloads or parses a grammar. Other languages use the existing Typst/Syntect
syntax bundle (bat/two-face); HTML and PDF share Grapharch's light palette.
