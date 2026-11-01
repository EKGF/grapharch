---
title: Code examples and syntax colours
parent: Usage
nav_order: 16
permalink: /usage/code-highlighting/
grapharch:
  subject: urn:grapharch:docs:usage:code-highlighting
---

# Code examples and syntax colours

Add a language after the opening Markdown fence. Grapharch colours code when it
builds the HTML and PDF, using the same light palette in both. The optional
`example` marker adds a numbered example caption.

````markdown
```python example
from pathlib import Path

print(Path("data").name)
```
````

| Language | Fence names |
| --- | --- |
| Python | `python`, `py` |
| Bash and shell | `bash`, `sh`, `shell` |
| JavaScript and Node.js | `javascript`, `js`, `node`, `nodejs` |
| TypeScript | `typescript`, `ts` |
| JSON | `json` |
| JSON-LD | `json-ld`, `jsonld` |
| Turtle | `turtle`, `ttl`, `rdf-turtle` |
| SPARQL | `sparql`, `rq` |

## JSON-LD and SPARQL

```json-ld example
{
  "@context": { "name": "https://schema.org/name" },
  "@id": "https://example.org/products/bonds",
  "name": "UK Bonds"
}
```

```sparql example
PREFIX dct: <http://purl.org/dc/terms/>
SELECT ?dataset $title WHERE {
  ?dataset dct:title $title .
  FILTER(LANG($title) = "en")
}
```

See the [multilingual code showcase](../showcases.md) for complete Python, Bash,
JavaScript, TypeScript, JSON, JSON-LD, Turtle and SPARQL examples with a matching PDF.
Its [publication definition](https://github.com/EKGF/grapharch/tree/main/examples/code-highlighting)
also demonstrates fenced Markdown stored in an RDF documentation fragment.

## Source and display

Code is displayed, never executed. Strings, Unicode, environment variables,
backticks, JSON-LD keywords and SPARQL IRIs remain source text. PDF code wraps
within the page; tabs display at four-column stops. Wrapped lines retain their
syntax colours, including inside multiline strings.

Grapharch also recognises languages in its bundled syntax library. Fences without
a language, or with an unrecognised language name, display as plain code. A fence
language controls presentation and does not validate the code.

Highlighting works offline: the generated website needs no syntax-highlighting
script or external service, and the PDF needs no additional Typst package. The
engine uses [Syntect](https://docs.rs/syntect/5.2.0/syntect/) and the syntax bundle
already used by Typst; SPARQL and Turtle use the bundled
[LinkedData grammar](https://github.com/blake-regalia/linked-data.syntaxes/releases/tag/v1.0.4).
