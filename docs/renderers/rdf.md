---
title: RDF
parent: Renderers
nav_order: 7
permalink: /renderers/rdf/
grapharch:
  subject: urn:grapharch:docs:renderers:rdf
---

# RDF renderer

Publishes collected RDF in the official RDF formats, one file per format.

## Output

A target that selects `ga:RdfRenderer` names the `ga:RdfFileCollector` uses it
publishes (`ga:rdfSource`) and the RDF formats it publishes them in
(`ga:rdfFormat`). Each format produces one file, `<ga:slug>.<extension>`, in the
target's destination directory:

```turtle
ex:project ga:target ex:vocabulary-source, ex:vocabulary-all .
ex:vocabulary-source a ga:Target ;
    ga:renderer ga:RdfRenderer ; ga:destination "rdf" ; ga:slug "vocabulary" ;
    ga:rdfSource ex:ontology-source ; ga:rdfFormat ga:SourceTextFormat .
ex:vocabulary-all a ga:Target ;
    ga:renderer ga:RdfRenderer ; ga:destination "rdf" ; ga:slug "vocabulary-all" ;
    ga:rdfSource ex:ontology-source, ex:shapes-source ;
    ga:rdfFormat ga:TurtleFormat, ga:NTriplesFormat, ga:NQuadsFormat,
        ga:TriGFormat, ga:JsonLdFormat, ga:RdfXmlFormat .
```

This publishes `rdf/vocabulary.ttl` and `rdf/vocabulary-all.ttl`, `.nt`, `.nq`,
`.trig`, `.jsonld` and `.rdf`. An RDF target declares no `ga:scope` or
`ga:profile`: it publishes its sources, not an arrangement of documentation.

## Formats

Format IRIs are defined in the [RDF format catalogue](../other/ontology/rdf-formats.ttl).
Each declares its media type and file extension in RDF; the renderer reads both
from the knowledge graph and selects the serializer that the media type names.

| Format | Media type | Extension |
| --- | --- | --- |
| `ga:TurtleFormat` | `text/turtle` | `ttl` |
| `ga:NTriplesFormat` | `application/n-triples` | `nt` |
| `ga:NQuadsFormat` | `application/n-quads` | `nq` |
| `ga:TriGFormat` | `application/trig` | `trig` |
| `ga:JsonLdFormat` | `application/ld+json` | `jsonld` |
| `ga:RdfXmlFormat` | `application/rdf+xml` | `rdf` |
| `ga:SourceTextFormat` | that of the collected source | that of the collected source |

`ga:SourceTextFormat` publishes the single source's exact collected bytes, named
and served as the RDF format the source was collected as. The other formats
publish the merged statements of all sources, in a deterministic order, with the
sources' prefixes. Turtle, N-Triples and RDF/XML cannot represent named graphs;
merged sources with named graphs fail in those formats.

A collector use named by `ga:rdfSource` retains its source text in the
knowledge graph. The files are therefore rebuilt from a restored graph without
the original repository, like every other renderer's output.

## Shared directories

An RDF target owns only its named files, like a [PDF](pdf.md) target. Several
RDF and PDF targets may share one destination directory; a file name that
another target already publishes fails the publication. A directory that a
directory publication, such as a specification, owns cannot hold them. Link the
files relatively from that publication, for example
`href="../rdf/vocabulary.ttl"`. Republishing a target withdraws the files of
formats it no longer declares.

## Content negotiation

An RDF target can declare a negotiated path, such as the path of the published
ontology IRI, and the target whose entry document that path serves by default:

```turtle
ex:vocabulary-all ga:negotiatedPath "ontology" ;
    ga:negotiatedHtml ex:specification .
```

A [hosting platform with routing](../usage/static-hosting.md#rdf-files-and-content-negotiation)
then serves the RDF file whose media type the request's `Accept` header names,
and the HTML otherwise. A negotiated target publishes each media type once.

## Failure cases

The publication fails, rather than omitting a file, when a source is not a
`ga:RdfFileCollector` use, `ga:SourceTextFormat` is combined with several
sources, two formats name the same file, a format is not a declared
`ga:RdfFormat`, `ga:slug` is missing or invalid, a negotiated path lacks its
HTML target (or that target has no entry document), or a target mixes RDF and
scope-arranging properties.
