---
title: Taxonomy publication research
parent: Architecture
nav_order: 3
permalink: /other/architecture/taxonomy-publication/
grapharch:
  subject: urn:grapharch:docs:topic:other:architecture:taxonomy-publication
---

# Publishing taxonomies for people

**Research and proposal, reviewed 31 October 2026.** This is a recommendation for
future publication work, not an accepted ADR, implementation contract, or claim
that these interfaces already exist. The current
[SKOS publication guide](../../usage/skos-taxonomies.md) describes GraphArch's
resource-reference output. That output preserves useful information, but a table
of RDF predicates is not the best primary interface for finding and understanding
terms.

The proposed direction is a dedicated vocabulary publication: a searchable web
reference with several ways to find concepts, paired with a deliberately arranged
print reference. Both would use the same pinned documentation KG. The web page
layout should not determine the reading order of the PDF.

## What established practice suggests

Getty's AAT help combines term searching, alphabetical results, hierarchical
context and full concept records. It distinguishes a matching variant from the
preferred term in search results, and shows contextual information to distinguish
homographs. These are useful precedents for readers who know a term and readers
who only know its broader subject. [Getty AAT online help](https://www.getty.edu/research/tools/vocabularies/aat/help.html).

Getty's *Introduction to Controlled Vocabularies*, by Patricia Harpring (2010),
separates the needs of vocabulary creators, indexers and general readers. Sections
7.5–7.5.3.5, printed pages 144–156, discuss multiple views, introductory guidance,
indented hierarchies, flat relationship notation and full records. Its
polyhierarchy examples retain multiple parent contexts. The discussion also
warns that unexplained specialist abbreviations can obstruct general readers.
[Getty book, chapter 7](https://d2aohiyo3d3idm.cloudfront.net/publications/virtuallibrary/160606018X.pdf).

FAO describes the human-facing AGROVOC service as a place to search concepts and
navigate their hierarchy, alongside separate downloads and machine interfaces.
This supports keeping human navigation distinct from RDF inspection.
[FAO: Access to AGROVOC](https://aims.fao.org/standards/agrovoc/access-agrovoc).

Skosmos is a useful implementation reference: its stated audience includes
indexers selecting terms and searchers finding keywords. It supplies a human
interface as well as API and linked-data access. Its use by Finto provides another
public reference to evaluate with vocabulary maintainers.
[Skosmos project](https://github.com/NatLibFi/Skosmos),
[Finto's YSO vocabulary](https://finto.fi/yso/en/).

These examples establish precedents, not proof that every taxonomist wants the
same layout. Before implementation, representative maintainers and readers should
try the proposed views with their own vocabulary.

## Proposed web publication

Offer three complementary entry points into each selected scheme:

| Entry point | Reader's task | Proposed presentation |
|---|---|---|
| Browse hierarchy | Explore an unfamiliar subject | Expandable branches with preferred labels, notations where supplied, and links to concept records |
| A–Z | Find a known term | A language-specific alphabetical index containing preferred and alternative labels, with clear pointers to the preferred entry |
| Search | Find a term despite different wording | Search preferred, alternative and hidden labels; show the matched wording and the preferred concept label |

A scheme landing page should explain scope, ownership, release, available
languages and how to use the vocabulary. It should offer these entry points,
source downloads and the matching PDF. Counts belong in supporting information;
they should not displace the explanation of what the vocabulary covers.

Each concept should have one stable record URL. Its main content should lead with
the preferred label and any supplied notation, then definition or scope note.
Readable sections should expose alternative labels, broader concepts, narrower
concepts, related concepts, examples and external mappings when supplied. Source
and revision details should remain accessible, with the IRI and RDF property
view in an advanced details area. This is a proposed presentation of existing
assertions, not permission to invent missing definitions.

For language handling, provide an explicit display-language choice and show
which language a label or note uses. Search may match an alternative spelling
without promoting it to preferred status. Hidden labels should aid retrieval
without becoming ordinary display synonyms. Missing translations should be
visible; any cross-language display policy should be explicit and configurable.
SKOS distinguishes preferred, alternative and hidden labels, and includes several
types of documentation note. [W3C SKOS Primer, labels and notes](https://www.w3.org/TR/skos-primer/#seclabel).

Hierarchy navigation must retain every supplied broader relationship. A concept
with two parents should be reachable through both contexts while sharing one
record. A selected navigation path can provide a breadcrumb; it must not imply
that other paths disappeared. Branches need bounded expansion, keyboard access
and clear links to additional contexts. A graph diagram can supplement this view;
it should not be the only navigation mechanism.

## Proposed PDF publication

A useful print edition should be a reference work rather than a printout of the
web interface. The following arrangement is our proposal, informed by the Getty
presentation conventions above:

1. **Front matter:** title, authority, release and KG revision, scope, language,
   editorial policy, license and a short guide to using the vocabulary.
2. **Systematic outline:** a compact indented hierarchy or classified arrangement
   that gives readers a map of the subject. Use supplied notations when suitable;
   do not manufacture classification codes.
3. **Alphabetical dictionary:** the detailed concept entries, with scope notes,
   preferred and alternative terms, relationships and stable identifiers.
4. **Additional indexes:** other publication languages, notations or collections
   when selected by the publisher and useful to the audience.
5. **Quality issues:** unresolved source findings with affected entries and
   provenance, separated from the vocabulary's normative content.

For a specialist thesaurus profile, introduce a legend and use familiar compact
relationship markers:

| Marker | Meaning in this proposed edition |
|---|---|
| USE | From an alternative label, consult the preferred label in this language |
| UF | Alternative wording associated with this preferred entry |
| BT | Broader concept |
| NT | Narrower concept |
| RT | Related concept |
| SN | Scope note |

For general readers, print the full phrases or pair them with abbreviations.
An alternative-label entry might read “Offering — USE Product, p. 42”; the
preferred entry would list “Alternative terms: Offering”. This is an illustrative
publication convention, not an assertion that two separately identified concepts
are equivalent.

Generate page references and hyperlinks from stable concept anchors after layout.
Keep a heading with the opening lines of its entry, use hanging indents for
relationships and readable running heads for the current alphabetical range.
A compact two-column dictionary may suit short entries; long definitions and
multilingual material need more room. Compare both with real content before
choosing a default.

In a polyhierarchy, the outline may repeat a concept's short label under each
parent and point to one detailed dictionary entry. This avoids both losing a
parent relationship and duplicating pages of definitions. For large publications,
let publishers select schemes, languages or explicitly curated divisions as
volumes. Splitting by arbitrary concept counts should not create invented
semantic modules.

## Preserve the kind of vocabulary actually supplied

SKOS represents more than strict thesauri: taxonomies, classifications and other
knowledge organization systems can use it. A flat vocabulary should therefore
remain usable through search and A–Z without an invented hierarchy. Concept
schemes, concepts, collections and ordered collections have different roles;
a collection's membership is not a broader/narrower relationship.
[W3C SKOS Primer, overview and collections](https://www.w3.org/TR/skos-primer/#seccollections).

The proposed renderer should keep these distinctions:

- Display direct hierarchy links separately from transitive ancestry.
- Keep associative links separate from hierarchy, and external mappings separate
  from local relationships.
- Preserve ordered collection member order in a dedicated collection view.
- Retain all supplied broader paths rather than choosing an arbitrary parent.
- Detect cycles during traversal, stop repeated expansion, and report the finding.
  A cycle is not automatically a violation of SKOS integrity conditions.
- Distinguish missing publication content from a violated semantic constraint.
  Multiple preferred labels for one language require different treatment from
  legitimate preferred labels in different languages.

These distinctions follow the SKOS data model; presentation diagnostics should
name the applicable publication policy or SKOS condition rather than calling
every unusual graph invalid. [W3C SKOS Reference, lexical labels, semantic
relations and collections](https://www.w3.org/TR/skos-reference/).

## What to validate before implementation

A prototype should answer concrete reader tasks: find a preferred term from its
synonym; understand a scope note; locate every broader context of a concept;
switch language without losing the concept; and follow a printed relationship
to its full entry. Test with maintainers, indexers and readers unfamiliar with RDF.

Acceptance examples should include a flat scheme, a multilingual scheme with
missing translations, polyhierarchy, an ordered collection, external mappings,
a cycle and a large scheme. Verify that no concept or relationship vanishes,
that search explains alternative-label matches, and that printed references
resolve after pagination changes. A restored KG with original sources removed
must produce the same publication content.

Implementation would also need bounded branch and search loading, a declared
sorting/collation policy, explicit publication limits, accessible controls and
reported build timings. These are proposed review criteria; this note does not
introduce new renderer names, CLI options or GraphArch annotation predicates.
