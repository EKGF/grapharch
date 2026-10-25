# Native DPROD reference showcase

Pinned from the user-owned sibling `../dprod` repository at
`89de8ff5c95149d79120f4062fcf5d1e0a82e641`. The unchanged source files and their
SHA-256 digests are listed in `reference.yaml`, with the upstream license retained.

The showcase collects the ontology, SHACL shapes, and authored Markdown, generates
53 annotated term fragments from RDF, and publishes three independent targets
from one assembled KG revision:

- `specification/`: the ontology and SHACL reference, with standards front matter.
- `website/`: a companion site with an introduction, story, and getting-started guide.
- `pdf/`: a review document with its own profile and the worked example as an appendix.

The companion prose is collected as logical Markdown topics and excluded from the
reference profiles. The PDF reuses the reference fragments with independent
placements. The specification renderer also retains its existing paired-PDF
rendition for its download button.

The pipeline It does not run ReSpec or wrap its generated HTML.
The upstream working copy is never modified.

Run `cargo run --example publish_showcase -- --dprod` or open this case in the
self-generated documentation gallery. The integration test deletes copied sources,
exports/restores the dataset, and renders from the restored KG.

Coverage: four OWL classes, thirteen object properties, one datatype property,
one SKOS concept scheme, six node shapes, and twenty-eight property shapes.
This is a native reference showcase, not complete DPROD specification acceptance.
