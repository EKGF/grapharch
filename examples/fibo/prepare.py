#!/usr/bin/env python3
"""Prepare external GraphArch benchmark hints; never copy or edit FIBO sources."""
import argparse
import hashlib
import json
import subprocess
import time
import urllib.parse
import xml.etree.ElementTree as ET
from pathlib import Path

RDF = "{http://www.w3.org/1999/02/22-rdf-syntax-ns#}"
OWL = "{http://www.w3.org/2002/07/owl#}"
XML = "{http://www.w3.org/XML/1998/namespace}"


def prepare(source: Path, output: Path, title: str, base_url: str, renderer: str = "specification") -> dict:
    started = time.perf_counter()
    source = source.resolve(strict=True)
    if not source.is_dir():
        raise ValueError("source must name the existing FIBO clone or a subtree")
    commit = subprocess.run(["git", "-C", str(source), "rev-parse", "HEAD"],
                            check=True, capture_output=True, text=True).stdout.strip()
    status = subprocess.run(["git", "-C", str(source), "status", "--porcelain"],
                            check=True, capture_output=True, text=True).stdout
    inventories = []
    declarations = []
    seen_iris = set()
    seen_prefixes = set()
    for path in sorted(source.rglob("*.rdf")):
        if ".git" in path.parts:
            continue
        if path.is_symlink():
            raise ValueError(f"symlink RDF source: {path}")
        bindings = [binding for _, binding in ET.iterparse(path, events=["start-ns"])]
        root = ET.parse(path).getroot()
        ontologies = root.findall(OWL + "Ontology")
        if len(ontologies) != 1:
            raise ValueError(f"expected one ontology in {path}")
        iri = urllib.parse.urljoin(root.get(XML + "base", ""),
                                   ontologies[0].get(RDF + "about", ""))
        prefixes = [prefix for prefix, namespace in bindings if namespace == iri]
        if len(prefixes) != 1 or not prefixes[0]:
            raise ValueError(f"expected one native namespace prefix for {iri}")
        if iri in seen_iris or prefixes[0] in seen_prefixes:
            raise ValueError(f"ambiguous ontology identity or prefix: {iri}")
        seen_iris.add(iri)
        seen_prefixes.add(prefixes[0])
        declarations.append(
            f"<{iri}> gov:inFamily <urn:fibo-benchmark:family>; "
            f"sh:declare [sh:prefix {json.dumps(prefixes[0])}; "
            f"sh:namespace {json.dumps(iri)}] .")
        data = path.read_bytes()
        inventories.append({"path": path.relative_to(source).as_posix(),
                            "ontology": iri, "prefix": prefixes[0],
                            "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    if not inventories:
        raise ValueError("source contains no FIBO RDF/XML ontology files")
    # This is an explicit benchmark grouping, not an assertion of approval or a
    # replacement for FIBO's native Module / dct:hasPart hierarchy.
    settings = f'''@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:fibo-benchmark:project> a ga:Project; dct:title {json.dumps(title)};
    ga:repository <https://github.com/edmcouncil/fibo>; ga:target <urn:fibo-benchmark:target> .
<urn:fibo-benchmark:scope> a ga:Scope .
<urn:fibo-benchmark:profile> a ga:Profile; pub:publishesFamily <urn:fibo-benchmark:family> .
<urn:fibo-benchmark:family> a gov:Family; rdfs:label {json.dumps(title)} .
<urn:fibo-benchmark:target> a ga:Target; dct:title {json.dumps(title)};
    dct:description "Local documentation and performance preview of the selected FIBO sources.";
    rdfs:comment "Unofficial GraphArch documentation preview. No approval is implied, imports are not fetched, and no reasoning is performed.";
    ga:scope <urn:fibo-benchmark:scope>; ga:profile <urn:fibo-benchmark:profile>;
    ga:renderer "{renderer}"; ga:destination "specification"; ga:tocDepth 4;
    ga:publicationStatus "Documentation Preview";
    ga:publicationDate "2026-10-26"^^<http://www.w3.org/2001/XMLSchema#date>;
    ga:thisVersion <{base_url}>; ga:latestVersion <{base_url}>;
    ga:license <https://opensource.org/licenses/MIT>;
    ga:publisher <urn:fibo-benchmark:authority> .
<urn:fibo-benchmark:authority> a ga:Authority; rdfs:label "EDM Association";
    ga:shortName "EDM Association"; ga:accentColor "#2b5f9f" .
'''
    output.mkdir(parents=True, exist_ok=True)
    (output / "publication.ttl").write_text(settings + "\n".join(declarations) + "\n")
    result = {"repository": "https://github.com/edmcouncil/fibo", "commit": commit,
              "working_tree_clean": not status, "source": str(source),
              "ontology_count": len(inventories),
              "rdfxml_bytes": sum(item["bytes"] for item in inventories),
              "preparation_seconds": time.perf_counter() - started, "files": inventories}
    (output / "source-manifest.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("../fibo"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--title", default="Financial Industry Business Ontology (FIBO)")
    parser.add_argument("--renderer", choices=("specification", "specification-pages", "bookshelf"), default="specification")
    parser.add_argument("--base-url", default="http://localhost:8872/specification/")
    args = parser.parse_args()
    url = urllib.parse.urlsplit(args.base_url)
    if url.scheme not in {"http", "https"} or not url.netloc or any(c in args.base_url for c in "<>\n\r"):
        raise ValueError("base-url must be an HTTP(S) publication URL")
    result = prepare(args.source, args.output, args.title, args.base_url, args.renderer)
    print(json.dumps({key: value for key, value in result.items() if key != "files"}))


if __name__ == "__main__":
    main()
