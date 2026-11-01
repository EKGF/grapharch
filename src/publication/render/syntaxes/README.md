The SPARQL and Turtle grammars are vendored unchanged from [LinkedData 1.0.4](https://github.com/blake-regalia/linked-data.syntaxes/releases/tag/v1.0.4),
`assets/{sparql,turtle}.sublime-syntax` in `LinkedData.sublime-package` (18 November 2021).
They cover SPARQL 1.1, SPARQL-star, Turtle and Turtle-star. Their ISC license is retained in
`packaging/licenses/linked-data-syntaxes.txt`.

`build.rs` compiles it with Syntect into an embedded syntax set. Publishing never
downloads or parses a grammar. Other languages use the existing Typst/Syntect
syntax bundle (bat/two-face); HTML and PDF share Grapharch's light palette.
