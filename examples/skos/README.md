# SKOS taxonomy showcase

Run `cargo run --example publish_showcase -- --skos` from the repository root.
The default output is `output/pdf/grapharch-skos`, containing a specification
website, PDF, and restorable `documentation.nq`.

This original, fictional vocabulary demonstrates native SKOS concepts, labels,
definitions, mappings and an ordered collection, with concepts split across RDF
files. Publication hints and authority metadata are in `project.ttl`. No external
ontology imports are fetched. Source data is offered under CC0, as declared by
the publication. Generated output stays ignored; CI regenerates the showcase.
