---
title: Ontology families
parent: Usage
nav_order: 7
permalink: /usage/ontology-families/
grapharch:
  subject: urn:grapharch:docs:topic:ontology-families
---

# Publish an ontology family

GraphArch can publish a family of related ontologies from a directory, a local
Git checkout, or a remote Git repository. The same collectors and renderers work
with your own ontologies, namespaces, family registry, and publishing authority.
Family membership and publication identities come from your project configuration.

A family publication keeps each ontology's identity and namespace, generates an
overview with descriptions and chapter links, and resolves references across the
family. Its documentation KG can produce a standards-style specification and a
PDF, alongside a separately configured companion website.

## Choose your source

Use `publication build-family` with one source option:

| Source | Option | Contents read |
| --- | --- | --- |
| Directory | `--source ./ontologies` | RDF files in that directory tree |
| Local Git checkout | `--local-repository /path/to/your-repo` | Working-tree files, including uncommitted edits |
| Remote Git repository | `--git-repository https://git.example.org/team/ontologies.git --git-ref main` | Files at one resolved commit |
| GitHub repository | `--github OWNER/REPO --git-ref main` | The same remote Git mode, with a GitHub URL shortcut |

When the source RDF contains one unambiguous `ga:Project` and `ga:Target`,
GraphArch discovers the publication hints directly:

```sh
grapharch publication build-family \
  --local-repository /path/to/your-repo \
  --output-root ./output/family
```

Hints can live inside an ontology or in a separate RDF file in the same
repository. The source owns its title, authority, family membership, and
publication settings. Multiple projects or targets require an explicit selection;
GraphArch does not choose one arbitrarily.

For external publication definitions, supply all three override options:

```sh
grapharch publication build-family \
  --local-repository /path/to/your-repo \
  --definitions ./publication.ttl \
  --project urn:example:project \
  --target urn:example:specification \
  --output-root ./output/family
```

Replace the paths and identifiers with your own. The project and target must be
defined in `publication.ttl`. Generated files stay under the chosen output root;
the source repository is not modified. With a target destination of
`specification`, outputs include `specification/index.html`, its PDF and Typst
source, and `documentation.nq`.

For a remote source, replace the source option with `--git-repository URL
--git-ref REF`, or the GitHub shortcut. Unqualified refs mean branches; use
`refs/tags/<tag>` for a tag or a full commit ID for an immutable selection.
If `--git-ref` is omitted, GraphArch follows the remote default branch (`HEAD`).
Each build resolves that moving ref once and records the concrete commit in the
documentation KG. The cached bare repository is refreshed on subsequent runs.

## Describe your family

Family identity and membership use the reusable Artifact Governance vocabulary;
publication profiles use the Artifact Publication vocabulary. You can provide
these relationships in your own registry, the ontology files, or the separate
publication definitions. They do not have to come from a central registry.

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix artgov: <https://ekgf.org/ontology/artifact-governance#> .
@prefix artpub: <https://ekgf.org/ontology/artifact-publication#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix sh: <http://www.w3.org/ns/shacl#> .

<urn:example:family> a artgov:Family ;
    rdfs:label "Example ontology family"@en .

<urn:example:profile> a ga:Profile, artpub:PublicationProfile ;
    artpub:publishesFamily <urn:example:family> .

<https://example.org/ontology/core#>
    artgov:inFamily <urn:example:family> ;
    sh:declare [
        sh:prefix "core" ;
        sh:namespace "https://example.org/ontology/core#"
    ] .
```

This illustrates family relationships and namespace hints, not a complete
publication configuration. Use your actual ontology IRIs and existing registered
identities where available. Configure the project, target, scope, publication
metadata, and authority as described in the
[publication pipeline](publication-pipeline.md) and
[specification renderer](specification-renderer.md) guides.

Membership can also be asserted through `artgov:Reference` resources using
`artgov:inFamily` and `artgov:refersTo`; explicit IRI aliases are honored. Missing
registered members fail the build, preventing a silently incomplete publication.

The current builder requires explicit membership and namespace declarations;
it does not infer a family from an arbitrary folder or namespace prefix. Each
ontology source file must declare one named ontology. Each ontology needs one
unambiguous English `rdfs:label` for its chapter title. Namespace and prefix hints
use `sh:declare` and can be supplied separately without editing the source files.

## What readers see

The opening overview provides ontology names, supplied descriptions, and quick
links to the vocabulary chapters. Resource counts and annotation diagnostics
appear in a documentation coverage appendix. Chapter titles use English ontology
labels; prefixes remain available in metadata and technical identifiers.

The reference documents classes, properties, named individuals, and named SHACL
constructs. Explicit `owl:NamedIndividual` resources have readable headings and
linked instance-type summaries. Resources also declared as classes or properties
retain those additional roles. The presentation reference is
[DCAT 3](https://www.w3.org/TR/vocab-dcat-3/).

Supplied English labels and definitions are reproduced. Missing resource
annotations appear as warnings and become `ga:Diagnostic` resources with source
and revision provenance. Rendering does not invent definitions, execute SHACL
validation, or fetch external ontology imports.

### Rich descriptions

Use `annotation:markdownDescription` from
`https://ekgf.org/ontology/annotation#` on an ontology, class, property, or
individual when its explanation needs Markdown:

```turtle
@prefix annotation: <https://ekgf.org/ontology/annotation#> .

<https://example.org/ontology/Account>
    annotation:markdownDescription """
## Scope

An account records **agreed responsibilities**.

- Identify the responsible party.
- Link the applicable agreement.
"""@en .
```

The reference renders headings, lists, links, fenced code, tables, and GitHub
Flavoured Markdown task lists. Headings nest beneath the resource's title.
The rich description replaces the plain description in the detailed reference;
without it, the existing source definitions and comments remain available.
An ontology's short comment still provides context in the family overview.

English publications select `@en` or untagged text. Identical rich
values are accepted; distinct competing values fail with an explicit error.
Descriptions in another language are not substituted for missing English text.
The original annotation and source provenance remain in the documentation graph.
Raw HTML and unsafe links fail rendering rather than executing or silently
losing content.

### Publishing authority

Your publishing authority is configurable. `ga:Authority` is equivalent to the
Artifact Governance `Authority`, a Commons party role. Its organization is a
separate resource linked with `cmns-rlcmp:isPlayedBy`. Display names come from
English `rdfs:label`; acronyms use `artgov:authorityAcronym` or its equivalent
`ga:shortName`. Conflicting values fail validation.

## Collection and caching

GraphArch reads Turtle, RDF/XML (`.rdf`), N-Triples, N-Quads, TriG, and N3.
XML catalog files are not treated as RDF sources.

Remote repositories are cached locally between runs. The default cache is under
the system temporary directory; use `--git-cache /path/to/cache` to choose another
location. Each run checks for updates and reads one fixed commit, without
creating a checkout. A failed update stops the build rather than publishing
outdated cached content.

GraphArch uses your Git credential configuration. Public repositories do not
require a GitHub API token.

The KG retains the selected commit's author, committer, and timestamps. Local
working-tree contents are identified independently from HEAD by the file
collectors. Renderers read the resolved plan and pinned KG rather than reopening
source files.

Current budgets are 4096 RDF files, 8000 documented family resources, 24 levels
of anonymous RDF structure, and the renderer's 10000 entries and 16 MiB content
limit. Unsupported structures and exceeded budgets are hard errors. Separate
specification, companion-site, and approval-PDF publications are described in
[publication families](publication-families.md).

## Examples and validation

These projects are examples and test cases for the general pipeline:

- **EKGF ontology family:** the gallery follows the remote repository's latest
  default branch and records its resolved commit in the downloadable KG. A separate
  offline fixture covers 24 ontologies and 704
  named resources. See
  [the examples guide](examples.md), or browse the
  [showcase gallery](../showcases.md). The copied corpus keeps regression tests
  reproducible without network access. Published gallery builds use the remote
  repository and fail if it cannot be fetched.
- **DPROD:** an individual-ontology showcase demonstrating a specification,
  companion website, and PDF. It is not a required vocabulary or input.
- **FIBO:** the large-family and native-module benchmark reads an existing clone;
  see `examples/fibo` for source manifests, setup, and timing instructions.

The same pipeline works with other organizations, repositories, and ontologies.
Tests also exercise independently defined families, namespaces,
and authorities, and verify rendering from the KG after sources are removed.


## Arrange each ontology reference

Each ontology chapter opens with an **Overview**, followed by nonempty **Classes**,
**Properties**, **Named Individuals**, **Shapes**, and **Other Terms** sections.
Term headings use supplied labels consistently; the prefixed identifier remains
in the entry. Classification uses explicit source types, without fetching imports
or inferring missing types. A punned named individual appears once under Named
Individuals and retains all its declared types in its entry.

Put GraphArch hints in the ontology source to rename the overview or rename and
reorder its resource sections:

```turtle
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .

<https://example.org/vocabulary> ga:overviewTitle "Summary"@en ;
    ga:referenceSection <https://example.org/docs/examples-section> .

<https://example.org/docs/examples-section> a ga:ReferenceSection ;
    ga:sectionKind "individuals" ;
    dct:title "Examples"@en ;
    ga:order 0 .
```

Section kinds are `classes`, `properties`, `individuals`, `shapes`, and `other`.
Their default orders are 10, 20, 30, 40, and 50. Overrides require a title and an
integer order from 0 to 1000, distinct from every other section's order. Empty
sections are omitted; overrides do not hide resources. Titles are plain text,
with one English or untagged value per section.

The ontology IRI identifies the ontology; its namespace prefixes vocabulary
terms. They may differ (for example, `https://example.org/vocabulary` and
`https://example.org/vocabulary#`). The overview shows them in adjacent rows
when different and combines them into one row when equal.


## Native modules and large families

Explicit FIBO `Module` instances and their `dct:hasPart` assertions become
`ga:OntologyModule` and `ga:moduleMember` facts alongside the unchanged native
assertions. Module nesting is preserved in the publication plan using
`ga:parentPlacement`. A domain can contain modules, which contain ontology
references and their Classes, Properties, Named Individuals, and Shapes sections.
The same hierarchy drives HTML numbering, contents navigation, and PDF chapters.

Containment is not inferred from directories or imports. Ontologies without
explicit module membership remain separate reference chapters. Missing members,
cycles, multiple parents, or more than eight component levels fail explicitly.
For headings deeper than HTML's six native heading tags, the specification
retains the semantic level using `aria-level` on an `h6` heading.

A target or profile can set `ga:tocDepth` to an integer from 1 to 8. A target's
value takes precedence. For example, depth 4 can list a large family's modules
and ontologies while leaving individual vocabulary entries out of the sidebar
and PDF outline. When absent, HTML keeps its complete contents and PDF retains
its depth of three.

Large-family budgets are finite and coordinated across the pipeline:

| Item | Limit |
| --- | ---: |
| Family resources, including ontology identities | 32,768 |
| Publication entries | 65,536 |
| Combined fragment content | 128 MiB |
| Link/reference index entries | 131,072 |
| Specification anchors | 262,144 |
| Specification links | 1,048,576 |
| Generated Typst source | 256 MiB |
| Individual retained artifact | 128 MiB |
| Retained showcase/website assets combined | 512 MiB |

Per-fragment, source-file, and concurrent-writer bounds still apply. The
[FIBO benchmark](https://github.com/EKGF/grapharch/tree/main/examples/fibo)
includes reproducible setup for an existing clone; no FIBO corpus is vendored.
