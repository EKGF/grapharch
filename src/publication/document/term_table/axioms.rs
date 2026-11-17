//! The axiom column of a term row: logical statements about one term in
//! description-logic notation, grouped under their OWL roles.
use {
    super::{
        expression::Expressions,
        kind::TermKind,
        names::TermNames,
        row::Entry,
    },
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
};

const OWL: &str = "http://www.w3.org/2002/07/owl#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";

/// Whether `predicate` states an axiom that the axiom column renders.
pub(super) fn is_axiom(predicate: &NamedNode) -> bool {
    matches!(
        predicate.as_str().strip_prefix(RDFS),
        Some("subClassOf" | "subPropertyOf" | "domain" | "range")
    ) || matches!(
        predicate.as_str().strip_prefix(OWL),
        Some(
            "equivalentClass" |
                "disjointWith" |
                "disjointUnionOf" |
                "hasKey" |
                "propertyChainAxiom" |
                "inverseOf" |
                "equivalentProperty" |
                "propertyDisjointWith" |
                "sameAs" |
                "differentFrom"
        )
    )
}

/// Collects one term's axiom entries in a fixed role order.
pub(super) struct Axioms<'a> {
    expressions: &'a Expressions<'a>,
    names:       &'a TermNames,
    kind:        TermKind,
    types:       Vec<String>,
    parents:     Vec<String>,
    anonymous:   Vec<Entry>,
    equivalents: Vec<String>,
    class:       Vec<Entry>,
    property:    Vec<Entry>,
    assertions:  Vec<Entry>,
}

impl<'a> Axioms<'a> {
    pub fn new(
        expressions: &'a Expressions<'a>,
        names: &'a TermNames,
        kind: TermKind,
    ) -> Self {
        Self {
            expressions,
            names,
            kind,
            types: Vec::new(),
            parents: Vec::new(),
            anonymous: Vec::new(),
            equivalents: Vec::new(),
            class: Vec::new(),
            property: Vec::new(),
            assertions: Vec::new(),
        }
    }

    /// A declared type beyond the term's kind: an individual's class or a
    /// property characteristic.
    pub async fn typed(&mut self, kind: &Term) -> Result<()> {
        let rendered = self.expressions.render(kind).await?.text;
        self.types.push(rendered);
        Ok(())
    }

    /// A property assertion about the term, `R = value`.
    pub async fn assertion(
        &mut self,
        property: &NamedNode,
        value: &Term,
    ) -> Result<()> {
        let value = self.expressions.render(value).await?.text;
        self.assertions.push(Entry::plain(format!(
            "{} = {value}",
            self.names.markdown(property)
        )));
        Ok(())
    }

    /// One axiom statement.
    pub async fn add(
        &mut self,
        predicate: &NamedNode,
        value: &Term,
    ) -> Result<()> {
        let rdfs = predicate.as_str().strip_prefix(RDFS);
        let owl = predicate.as_str().strip_prefix(OWL);
        let named = matches!(value, Term::NamedNode(_)) &&
            !crate::publication::document::anonymous_order::AnonymousOrder::is_anonymous(value);
        match (rdfs, owl) {
            (Some("subClassOf" | "subPropertyOf"), _) if named => {
                self.parents
                    .push(self.expressions.render(value).await?.text)
            },
            (Some("subClassOf"), _) => {
                let rendered = self.expressions.render(value).await?;
                self.anonymous.push(Entry::labelled(
                    if rendered.restriction {
                        "Property Restriction"
                    } else {
                        "Parent Class"
                    },
                    rendered.text,
                ));
            },
            (Some("subPropertyOf"), _) => {
                let rendered = self.expressions.render(value).await?.text;
                self.anonymous
                    .push(Entry::labelled("Parent Property", rendered));
            },
            (Some(role @ ("domain" | "range")), _) => {
                let rendered = self.expressions.render(value).await?.text;
                self.property.push(Entry::labelled(
                    if role == "domain" { "Domain" } else { "Range" },
                    rendered,
                ));
            },
            (_, Some("equivalentClass")) if named => {
                self.equivalents
                    .push(self.expressions.render(value).await?.text)
            },
            (_, Some("equivalentClass")) => {
                let rendered = self.expressions.render(value).await?.text;
                if self.kind == TermKind::Datatype {
                    self.equivalents.push(rendered);
                } else {
                    self.class.push(Entry::labelled(
                        "Class Axiom",
                        format!("≡ {rendered}"),
                    ));
                }
            },
            (_, Some("disjointWith")) => {
                let rendered = self.expressions.render(value).await?.text;
                self.class.push(Entry::labelled(
                    "Class Axiom",
                    format!("¬ {rendered}"),
                ));
            },
            (_, Some("disjointUnionOf")) => {
                let members =
                    self.expressions.named_list(value.clone()).await?;
                self.class.push(Entry::labelled(
                    "Disjoint Union",
                    members.join(" ⊔ "),
                ));
            },
            (_, Some("hasKey")) => {
                let members =
                    self.expressions.named_list(value.clone()).await?;
                self.class
                    .push(Entry::labelled("Has Key", members.join(", ")));
            },
            (_, Some("propertyChainAxiom")) => {
                let chain = self.expressions.chain(value.clone()).await?;
                self.property.push(Entry::labelled("Property Chain", chain));
            },
            (_, Some("inverseOf")) => {
                let rendered = self.expressions.render(value).await?.text;
                self.property.push(Entry::labelled("Inverse", rendered));
            },
            (_, Some("equivalentProperty")) => {
                let rendered = self.expressions.render(value).await?.text;
                self.property
                    .push(Entry::labelled("Equivalent Property", rendered));
            },
            (_, Some("propertyDisjointWith")) => {
                let rendered = self.expressions.render(value).await?.text;
                self.property.push(Entry::labelled(
                    "Property Axiom",
                    format!("¬ {rendered}"),
                ));
            },
            (_, Some(role @ ("sameAs" | "differentFrom"))) => {
                let rendered = self.expressions.render(value).await?.text;
                self.assertions.push(Entry::labelled(
                    if role == "sameAs" {
                        "Same As"
                    } else {
                        "Different From"
                    },
                    rendered,
                ));
            },
            _ => bail!("{predicate} is not a term-table axiom"),
        }
        Ok(())
    }

    /// The entries, in role order.
    pub fn entries(self) -> Vec<Entry> {
        let mut entries = Vec::new(); // bounded: the term's fact budget.
        if !self.types.is_empty() {
            entries.push(Entry::labelled("Type", self.types.join(", ")));
        }
        if !self.parents.is_empty() {
            entries.push(Entry::labelled(
                if self.kind == TermKind::Property {
                    "Parent Property"
                } else {
                    "Parent Class"
                },
                self.parents.join(", "),
            ));
        }
        entries.extend(self.anonymous);
        if !self.equivalents.is_empty() {
            entries.push(Entry::labelled(
                if self.kind == TermKind::Datatype {
                    "Equivalent Datatype"
                } else {
                    "Equivalent Class"
                },
                self.equivalents.join(", "),
            ));
        }
        entries.extend(self.class);
        entries.extend(self.property);
        entries.extend(self.assertions);
        entries
    }
}
