//! The reader-facing notation contract shared by every class diagram.
pub(crate) const LANGUAGE: &str = "grapharch-class-diagram";
pub(crate) const LEGEND: &str =
    "Class diagrams use UML shorthand for asserted OWL facts and declared \
     SHACL constraints, without inference or a claim that validation passed. \
     RDF properties remain independent resources even when drawn as class \
     members or labelled associations. Domain/range declarations alone do not \
     impose requiredness or cardinality. A property with a declared domain \
     and range is a labelled association from the domain to the range; a \
     datatype property, or one without a specific range, is an attribute of \
     its domain. Multiple domains or ranges apply conjunctively, not as \
     alternatives: each declared domain-range pair is drawn and holds for \
     every use of the property. An explicitly declared union domain is drawn \
     as one source box whose class alternatives are joined by or. A dotted \
     rdfs:Resource source box is a presentation placeholder when no specific \
     domain is declared in the selected inputs, not an asserted domain or a \
     claim that none can be entailed. Simple existential subclass \
     restrictions use a class-scoped association with target-end 1..* and \
     {qualified}, meaning at least one value in the named filler class; this \
     does not constrain all property values or declare a global range. \
     Datatype-qualified restrictions appear as class fields with the declared \
     datatype and bracketed count of distinct data values; they do not \
     declare a global property range. Qualified cardinality associations show \
     n..* for minima, n for exact counts, or 0..n for maxima at the named \
     target with {qualified}: only values in that target class count, and \
     other property values are not excluded. A zero maximum prohibits values \
     in that named target class; it is never omitted. A simple zero qualified \
     minimum imposes no restriction and is omitted visually. An existential \
     with a simple zero-minimum filler still requires a value; a positive \
     qualified minimum or exact count on the same class and property already \
     guarantees that existence. Unsupported anonymous OWL expressions remain \
     explicit. Types and counts from simple property shapes enrich the UML \
     notation. Identical qualifiers are shown once; where declaring shapes \
     disagree, the disputed type or multiplicity is omitted from the drawing \
     and remains in the dedicated shape documentation. No shape is chosen \
     arbitrarily and conflicting counts are not silently intersected. An \
     absent count imposes no lower or upper bound beyond zero or unbounded, \
     respectively. Complex constraints require the linked full shape \
     definition. Original OWL facts and shape provenance remain in the \
     published source graph. Some diagrams show at most five complete OWL \
     relationship groups within an eighteen-fact budget, selected in \
     deterministic IRI order; omitted groups remain in the published source \
     graph. These views are not exhaustive. Names from the same ontology as \
     the documented class are shown by label; names from another ontology add \
     their prefixed name in parentheses, and unlabelled names show their \
     prefixed name.";
