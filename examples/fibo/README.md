# FIBO scale and module benchmark

This experiment reads an existing clone of `edmcouncil/fibo` without copying its
ontologies into GraphArch or editing the clone. It prepares an external
publication project, then uses the normal collectors and specification renderer.
The output is an unofficial documentation preview, with EDM Association as the
configured publishing authority.

Use a GraphArch build containing the large-family and module changes. The
published `0.0.1` binary predates these changes and stops at its 8,000-resource
limit. For development measurements, build an optimized binary:

```sh
git clone https://github.com/edmcouncil/fibo.git ../fibo
cargo build --release --locked
python3 examples/fibo/prepare.py --source ../fibo --output output/fibo/config
```

The preparation script uses FIBO's native ontology IRIs and namespace bindings.
It records the checked-out commit, whether the working tree is clean, and a hash
of each RDF/XML file. The resulting `publication.ttl` supplies the publication
settings and an explicit benchmark family. FIBO's own modules and
`dct:hasPart` relationships are recognized by the Rust RDF collectors.

Run the full family, with Git provenance from the existing checkout:

```sh
grapharch publication build-family \
  --local-repository ../fibo \
  --definitions output/fibo/config/publication.ttl \
  --project urn:fibo-benchmark:project \
  --target urn:fibo-benchmark:target \
  --output-root output/fibo/full \
  --concurrency 4 --json
```

For the development binary, replace `grapharch` with `target/release/grapharch`.
Set `RUST_LOG=grapharch=info` to record collection, documentation, assembly, and
planning times. Use `/usr/bin/time -lp` on macOS or `/usr/bin/time -v` on Linux to
measure total wall time and maximum resident memory. Those totals include PDF
compilation and KG export, not just collection. Run workloads independently for
comparable timings, and distinguish clean output builds from persistent-store
updates and subsequent runs with warm filesystem caches.

The specification, PDF, Typst source, and restorable KG are written beneath the
output root. To preview using the default publication URL:

```sh
python3 -m http.server 8872 --directory output/fibo/full
```

Open `http://localhost:8872/specification/`. Change `--base-url` when preparing
settings for another host. `ga:tocDepth 4` lists the domain/module/ontology
hierarchy in the HTML contents and PDF outline without listing every term.

A smaller run is useful for iteration:

```sh
python3 examples/fibo/prepare.py --source ../fibo/FND \
  --title "FIBO Foundations" --output output/fibo/fnd-config

grapharch publication build-family \
  --source ../fibo/FND \
  --definitions output/fibo/fnd-config/publication.ttl \
  --project urn:fibo-benchmark:project \
  --target urn:fibo-benchmark:target \
  --output-root output/fibo/foundations --json
```

Directory collection supports a subtree; local repository mode names the clone
root. External imports are recorded as source assertions and are not fetched.
Ontologies without explicit module membership remain separate reference
chapters. Missing members, cycles, or ambiguous parentage are hard errors.

The native PDF performance regression is intentionally explicit because it must
run with optimization rather than turn debug-build overhead into a timing claim:

```sh
cargo test --release --lib large_book_running_headers -- --ignored
```


## Recorded full-family run

On 26 October 2026, the local clone was at
`2d73435a4419b66b7c9f44cb9763e35fe8ac7890`, with a clean working tree.
The measurement used an optimized local GraphArch build on an Apple M3 Max
with 128 GiB RAM, reading the existing checkout with warm filesystem caches.
It is an observed run, not a cross-machine performance guarantee.

| Measurement | Result |
| --- | ---: |
| Full clone, including Git history | 26.51 seconds |
| RDF/XML ontology files | 296 |
| RDF/XML source bytes | 13,282,878 |
| Explicit named resources in those ontologies | 16,283 |
| Native modules | 66 |
| Source collection | 0.425 seconds |
| Collection and fragment generation, cumulative | 7.872 seconds |
| Validated KG assembled, cumulative | 16.618 seconds |
| Publication plan resolved, cumulative | 19.532 seconds |
| Typst layout | 789.898 seconds |
| PDF encoding | 1.331 seconds |
| Complete command, including export | 863.38 seconds |
| Maximum resident memory | 14.41 GiB |
| PDF pages | 6,789 |
| PDF bytes | 41,663,905 |
| HTML bytes | 37,106,442 |

The original released binary stopped at its 8,000-resource limit. Increasing
that limit exposed a quadratic assembly head comparison and repeated planner
lookups. The larger book also exposed a per-page chapter scan in the PDF
running header. The regression fixes preserve validation, ordering, and header
selection; this measurement includes those fixes and native module nesting.

The monolithic PDF is an extreme stress workload, not a suitable mandatory
per-change showcase build. A normal CI path can check the full KG/HTML and
representative module PDFs; scheduled or manual optimized runs can measure the
entire PDF. This example does not silently add a 14-minute, 14-GiB job to the
default showcase gallery or claim that a previous PDF is a build of newer inputs.
