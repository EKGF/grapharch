//! The reader-facing notation contract shared by every class diagram.
pub(crate) const LANGUAGE: &str = "grapharch-class-diagram";
pub(crate) const LEGEND: &str =
    "Class diagrams use UML notation for asserted OWL facts and declared \
     SHACL constraints, without inference or a claim that validation passed. \
     Declared classes are plain class boxes; «external» marks a class used \
     but not declared in the selected inputs, «datatype» an RDF datatype, \
     «metaclass» the RDF class of classes (Class) or of properties \
     (Property), and «enumeration» a class enumerating its literals. A hollow \
     triangle points from a subclass to its superclass. {disjoint} on \
     generalizations to a shared parent, or on a dashed line between two \
     classes, states that they share no individuals; a dashed «equivalent» \
     line joins equivalent classes, and {complete} marks classes whose union \
     is their parent. RDF properties remain independent resources even when \
     drawn as class members or labelled associations. Domain/range \
     declarations alone do not impose requiredness or cardinality. A property \
     with a declared domain and range is a labelled association from the \
     domain to the range; a datatype property, or one without a specific \
     range, is an attribute of its domain. Multiple domains or ranges apply \
     conjunctively, not as alternatives: each declared domain-range pair is \
     drawn and holds for every use of the property. An explicitly declared \
     union domain is drawn from each of its classes, with {xor} on each \
     association. An association with a name at each end is a property and \
     its declared inverse. A dotted «any» source box is a presentation \
     placeholder when no specific domain is declared in the selected inputs, \
     not an asserted domain or a claim that none can be entailed. OWL class \
     restrictions on the documented class and on neighbouring classes use UML \
     notation instead of expression boxes, scoped to the restricted class and \
     never declaring a global range or count. A datatype restriction is an \
     attribute, name : type [multiplicity]; an object restriction is an \
     association to its filler class with the multiplicity at the target end \
     (a neighbouring class shows it as a typed attribute instead). An \
     unqualified count uses the declared property range. someValuesFrom gives \
     1..* {qualified}; allValuesFrom gives * {all values}, narrowing the \
     value type; a qualified count counts only values in the named filler and \
     adds {qualified}; hasValue adds {= value}. {defining} marks a \
     restriction stated by an equivalence, which therefore defines the class. \
     A union filler is one association per class with {xor}; a chain of \
     existentials is a path member such as p.q : C [1..*]; a restriction on \
     an inverse property is an association from its filler. Multiplicities \
     follow one convention: equal bounds show one number such as 1, an \
     unconstrained range shows *, and otherwise lower..upper such as 0..1 or \
     1..*. A maximum or exact count of zero, in OWL or SHACL, is a \
     prohibition rather than a multiplicity: it appears as a class constraint \
     row after the attributes, {no p}, or {no p in C} when only values in C \
     are excluded. A zero minimum imposes no restriction and is omitted. \
     {redefines} marks a restriction on a property whose declared domain is \
     an asserted superclass, redefining the inherited property of the same \
     name. An existential whose filler every individual satisfies still \
     requires a value, shown as p [1..*]; a positive count on the same class \
     and property already guarantees that existence. Any other class \
     expression is an OCL-style constraint note in plain text, such as {not \
     C} for a complement. Malformed OWL expressions keep their authored facts \
     as explicit boxes named after their form, such as Anonymous OWL \
     restriction 1. Types and counts from simple property shapes enrich the \
     UML notation. Identical qualifiers are shown once; where declaring \
     shapes disagree, the disputed type or multiplicity is omitted from the \
     drawing and remains in the dedicated shape documentation. No shape is \
     chosen arbitrarily and conflicting counts are not silently intersected. \
     An absent count imposes no lower or upper bound beyond zero or \
     unbounded, respectively. Complex constraints require the linked full \
     shape definition. Original OWL facts and shape provenance remain in the \
     published source graph. Some diagrams show at most five complete OWL \
     relationship groups within an eighteen-fact budget and at most eight \
     class expressions, selected in deterministic order; omitted groups \
     remain in the published source graph. These views are not exhaustive. \
     Names from the same ontology as the documented class are shown by label; \
     names from another ontology add their prefixed name in parentheses, and \
     unlabelled names show their prefixed name. Two boxes that would show the \
     same name each add a (from vocabulary) line.";
