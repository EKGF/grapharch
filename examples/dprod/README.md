# Native DPROD reference showcase

The pinned DPROD sources come from the user-owned sibling `../dprod` repository
at `89de8ff5c95149d79120f4062fcf5d1e0a82e641`. The unchanged source files and
their SHA-256 digests are recorded as `ga:copiedSource` entries in `showcase.ttl`, with the upstream license
retained.

The example is split the way an exact-revision build reads it:

- `content/` is the designated content tree. It carries no GraphArch
  configuration: the pinned ontology, SHACL shapes, worked example and license,
  the authored abstract, status and overview, and the companion `website/`
  pages.
- `definitions.ttl` holds every GraphArch publication definition: the project,
  its three targets and profiles, one collector use per content file, the
  scope inclusions, the vocabulary reference documentor with its reference
  sections, and the authored placements of both reference profiles.

Tests commit `content/` and `definitions.ttl` to two separate Git repositories
and build them with `grapharch publication build-revision`, or with the same
library pipeline, from three independent targets of one assembled KG revision:

- `specification/`: the ontology and SHACL reference, with standards front matter.
- `website/`: a companion site with an introduction, story, and getting-started guide.
- `pdf/`: a review document with its own profile and the worked example as an appendix.

The companion prose is collected as website pages and excluded from the
reference scope. The PDF reuses the reference fragments with independent
placements. The specification publication date is derived from the ontology's
`dct:issued` through `ga:metadataSource`. The specification renderer also
retains its existing paired-PDF rendition for its download button.

The pipeline does not run ReSpec or wrap its generated HTML, and never modifies
the upstream working copy. The integration tests delete the repositories and
cache, export and restore the dataset, and render from the restored KG.

`showcase.ttl` configures the gallery case against the upstream repository by
exact commits: the content commit, and the controlling commit holding DPROD's
own GraphArch definitions.

Coverage: four OWL classes, thirteen object properties, one datatype property,
one SKOS concept scheme, six node shapes, and twenty-eight property shapes.
This is a native reference showcase, not complete DPROD specification acceptance.
