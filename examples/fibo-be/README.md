# FIBO Business Entities

This showcase documents the **complete `BE/` directory** of the official
[EDM Council FIBO repository](https://github.com/edmcouncil/fibo/tree/33a92cd34285d532c158da97bb42add089f135d3/BE),
pinned to commit `33a92cd34285d532c158da97bb42add089f135d3`.

The explicit inventory contains all 48 RDF/XML documents (1,064,379 source bytes),
including domain and module metadata, aggregation ontologies and regional
reference individuals. The source declares 216 OWL classes, 84 object properties,
9 datatype properties and 651 named individuals. These are declaration counts,
not a claim of distinct inferred entities. Within the BE namespace, the inputs
explicitly declare 199 distinct classes and 92 distinct object/datatype
properties; the source files also repeat some external declarations.

The HTML publication has an overview and individual ontology pages. A combined
PDF and the retained documentation KG are also produced. Module membership comes
from FIBO's own `Module` and `dct:hasPart` assertions rather than a second manual
directory hierarchy.

## Scope and imports

The 43 distinct direct imports outside this BE inventory remain external
references. They belong to FIBO Foundations, OMG Commons and OMG LCC. The
collector does not fetch an implicit import closure or perform reasoning. This
is the full BE source module, **not the whole FIBO ontology**. Unavailable external
definitions are not invented or silently copied from another release.

## Source provenance and license

Every `ga:rdfDocument` IRI in `showcase.ttl` names the immutable Git commit. The RDF URL collector
retains each original source URL and content digest alongside its assertions.
The local files here contain only transport and publication configuration; no
upstream RDF is vendored. The original MIT notices, copyright holders and years
embedded in the ontologies remain in the documentation graph and publication.
See the pinned [upstream license](https://github.com/edmcouncil/fibo/blob/33a92cd34285d532c158da97bb42add089f135d3/LICENSE).

This is an unofficial GraphArch documentation preview, not an EDM Council or OMG
approved replacement specification.

## Run

The gallery builds this registered example automatically.

The live acceptance test collects all sources, exports the KG, deletes its local
configuration, restores into a fresh dataset, and renders HTML and PDF:

```sh
cargo test --test fibo_showcase live_fibo_be -- --ignored --nocapture
```

It requires network access and the configured PlantUML executable, just like the
other live ontology showcases. Failures block publication; there is no stale
artifact substitution.
