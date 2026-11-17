//! OWL restrictions and datatype restrictions in description-logic notation.
use {
    super::{
        expression::{Expressions, Rendered},
        rdf_list::RdfList,
    },
    crate::publication::document::anonymous_order::AnonymousOrder,
    anyhow::{Result, bail, ensure},
    oxrdf::Term,
};

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

impl Expressions<'_> {
    pub(super) async fn restriction(
        &self,
        restriction: &Restriction,
        form: &str,
        value: &Term,
        depth: usize,
    ) -> Result<Rendered> {
        let property = restriction.property.as_ref().ok_or_else(|| {
            anyhow::anyhow!("an OWL restriction needs owl:onProperty")
        })?;
        let property = self.boxed(property, depth + 1).await?.nested();
        let filler = |rendered: Rendered| {
            if rendered.compound {
                format!("{property} {}", rendered.nested())
            } else {
                format!("{property}.{}", rendered.text)
            }
        };
        let text = match form {
            "someValuesFrom" => {
                format!(
                    "∃ {}",
                    filler(self.boxed(value, depth + 1).await?)
                )
            },
            "allValuesFrom" => {
                format!(
                    "∀ {}",
                    filler(self.boxed(value, depth + 1).await?)
                )
            },
            "hasValue" => {
                format!(
                    "∃ {property}.{{{}}}",
                    self.boxed(value, depth + 1).await?.text
                )
            },
            "hasSelf" => format!("∃ {property}.Self"),
            _ => {
                let operator = if form.starts_with("min") {
                    "≥"
                } else if form.starts_with("max") {
                    "≤"
                } else {
                    "="
                };
                let Term::Literal(count) = value else {
                    bail!("an OWL cardinality must be a literal")
                };
                let count = count.value().parse::<u64>().map_err(|_| {
                    anyhow::anyhow!(
                        "an OWL cardinality must be a nonnegative integer"
                    )
                })?;
                let qualified = form.to_ascii_lowercase().contains("qualified");
                ensure!(
                    qualified == restriction.qualifier.is_some(),
                    "owl:onClass or owl:onDataRange belongs to qualified \
                     cardinalities"
                );
                match &restriction.qualifier {
                    Some(qualifier) => {
                        format!(
                            "{operator} {count} {}",
                            filler(self.boxed(qualifier, depth + 1).await?)
                        )
                    },
                    None => format!("{operator} {count} {property}"),
                }
            },
        };
        Ok(Rendered::of_restriction(text))
    }

    pub(super) async fn datatype_restriction(
        &self,
        datatype: &Term,
        facets: Option<Term>,
        depth: usize,
    ) -> Result<Rendered> {
        let base = self.boxed(datatype, depth + 1).await?.nested();
        let facets = facets.ok_or_else(|| {
            anyhow::anyhow!("owl:onDatatype needs owl:withRestrictions")
        })?;
        let mut parts = Vec::new(); // bounded: rdf_list::MEMBERS.
        for facet in RdfList::new(self.view, facets).members().await? {
            let Term::NamedNode(node) = &facet else {
                bail!("a datatype facet must be a resource")
            };
            let facts = AnonymousOrder::new(self.dataset, self.view)
                .facts(node)
                .await?;
            let [(predicate, Term::Literal(value))] = facts.as_slice() else {
                bail!("a datatype facet states exactly one literal constraint")
            };
            let symbol = match predicate.as_str().strip_prefix(XSD) {
                Some("minInclusive") => "≥".to_owned(),
                Some("maxInclusive") => "≤".to_owned(),
                Some("minExclusive") => ">".to_owned(),
                Some("maxExclusive") => "<".to_owned(),
                Some(other) => format!("xsd:{other}"),
                None => self.names.name(predicate),
            };
            parts.push(format!("{symbol} {}", Self::literal(value)));
        }
        Ok(Rendered::atom(format!(
            "{base}[{}]",
            parts.join(", ")
        )))
    }
}

/// The parts of an OWL restriction read from its facts.
#[derive(Default)]
pub(super) struct Restriction {
    pub property:  Option<Term>,
    pub qualifier: Option<Term>,
    pub form:      Option<(String, Term)>,
    pub datatype:  Option<Term>,
    pub facets:    Option<Term>,
}
