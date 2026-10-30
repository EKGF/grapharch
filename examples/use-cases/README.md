# EKGF use-case tree showcase

Run `cargo run --example publish_showcase -- --use-cases` from the repository root.
The default output is `output/pdf/grapharch-use-cases`: a reader website, review
PDF and restorable `documentation.nq`. Use a source build newer than 0.1.0.

The original fictional RDF uses the EKGF Use Case Ontology directly. There are
six actual UseCase instances, two ownership roots, reuse across branches, and
related persona, outcome, stereotype and concept resources. There is no fabricated
OWL ontology wrapping the instances, and no Markdown catalog conversion.

The reusable documentor reads the collected KG, checks the ownership forest,
and retains per-case SVG neighborhoods alongside generated fragments. Ownership
is solid and reuse dashed. Link lists provide navigation; images are static.
Diagram layout follows the presentation ideas in EKG Catalog's DIAGRAMS.md;
its Python code and catalog content are not copied. This fixture is MIT-licensed
with GraphArch. The native collector preserves the supplied EKGF vocabulary IRIs.

The gallery and its CI verification run this same pipeline. Tests delete source
files, restore the KG and regenerate HTML/PDF, and reject invalid ownership.
