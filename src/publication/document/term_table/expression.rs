//! OWL class, datatype and property expressions in description-logic
//! notation: ∃ ∀ ≥ ≤ = ⊓ ⊔ ¬, oneOf, inverse ⁻ and property chains ∘.
use {
    super::{names::TermNames, rdf_list::RdfList, restriction::Restriction},
    crate::publication::{
        dataset::Dataset,
        document::anonymous_order::AnonymousOrder,
        view::RevisionView,
    },
    anyhow::{Result, bail, ensure},
    futures::future::BoxFuture,
    oxrdf::{Literal, Term},
};

const OWL: &str = "http://www.w3.org/2002/07/owl#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
/// The deepest anonymous expression nesting rendered.
const DEPTH: usize = 24;

/// One rendered expression and whether it needs parentheses when nested.
pub(super) struct Rendered {
    pub text:            String,
    pub(super) compound: bool,
    /// Whether the expression is an OWL property restriction.
    pub restriction:     bool,
}

impl Rendered {
    pub(super) fn atom(text: String) -> Self {
        Self { text, compound: false, restriction: false }
    }

    pub(super) fn of_restriction(text: String) -> Self {
        Self { text, compound: false, restriction: true }
    }

    fn compound(text: String) -> Self {
        Self { text, compound: true, restriction: false }
    }

    /// The text, parenthesized when it is compound.
    pub(super) fn nested(self) -> String {
        if self.compound {
            format!("({})", self.text)
        } else {
            self.text
        }
    }
}

/// Renders the expressions of one source view.
pub(super) struct Expressions<'a> {
    pub(super) dataset: &'a Dataset,
    pub(super) view:    &'a RevisionView,
    pub(super) names:   &'a TermNames,
}

impl<'a> Expressions<'a> {
    pub fn new(
        dataset: &'a Dataset,
        view: &'a RevisionView,
        names: &'a TermNames,
    ) -> Self {
        Self { dataset, view, names }
    }

    /// Render a class, datatype or individual value.
    pub async fn render(&self, value: &Term) -> Result<Rendered> {
        self.boxed(value, 0).await
    }

    async fn render_at(&self, value: &Term, depth: usize) -> Result<Rendered> {
        ensure!(
            depth <= DEPTH,
            "OWL expression exceeds nesting depth {DEPTH}"
        );
        match value {
            Term::NamedNode(node) if AnonymousOrder::is_anonymous(value) => {
                let facts = AnonymousOrder::new(self.dataset, self.view)
                    .facts(node)
                    .await?;
                self.anonymous(&facts, depth).await
            },
            Term::NamedNode(node) => {
                Ok(Rendered::atom(self.names.markdown(node)))
            },
            Term::Literal(literal) => {
                Ok(Rendered::atom(Self::literal(literal)))
            },
            _ => bail!("unsupported RDF term in an OWL expression"),
        }
    }

    pub(super) fn boxed<'b>(
        &'b self,
        value: &'b Term,
        depth: usize,
    ) -> BoxFuture<'b, Result<Rendered>> {
        Box::pin(self.render_at(value, depth))
    }

    /// A data value, quoted, with its language or non-string datatype.
    pub fn literal(literal: &Literal) -> String {
        let text = super::super::annotated_term::escape(literal.value());
        if let Some(language) = literal.language() {
            format!("\"{text}\"@{language}")
        } else if literal.datatype().as_str() == format!("{XSD}string") {
            format!("\"{text}\"")
        } else {
            format!(
                "\"{text}\"^^{}",
                super::super::annotated_term::escape(
                    &literal
                        .datatype()
                        .as_str()
                        .strip_prefix(XSD)
                        .map(|local| format!("xsd:{local}"))
                        .unwrap_or_else(|| {
                            literal.datatype().as_str().to_owned()
                        }),
                )
            )
        }
    }

    async fn anonymous(
        &self,
        facts: &[(oxrdf::NamedNode, Term)],
        depth: usize,
    ) -> Result<Rendered> {
        let mut construct = None;
        let mut restriction = Restriction::default();
        for (predicate, value) in facts {
            let local = predicate.as_str().strip_prefix(OWL);
            match local {
                _ if predicate.as_str() == RDF_TYPE => {
                    let Term::NamedNode(kind) = value else {
                        bail!("an expression type must be an IRI")
                    };
                    ensure!(
                        matches!(
                            kind.as_str().strip_prefix(OWL),
                            Some("Restriction" | "Class")
                        ) || kind.as_str() ==
                            "http://www.w3.org/2000/01/rdf-schema#Datatype",
                        "unsupported OWL expression type {kind}"
                    );
                },
                Some("onProperty") => {
                    restriction.property = Some(value.clone())
                },
                Some("onClass" | "onDataRange") => {
                    restriction.qualifier = Some(value.clone())
                },
                Some(
                    "someValuesFrom" |
                    "allValuesFrom" |
                    "hasValue" |
                    "hasSelf" |
                    "minCardinality" |
                    "maxCardinality" |
                    "cardinality" |
                    "minQualifiedCardinality" |
                    "maxQualifiedCardinality" |
                    "qualifiedCardinality",
                ) => {
                    ensure!(
                        restriction.form.is_none(),
                        "an OWL restriction needs exactly one constraint"
                    );
                    restriction.form = Some((
                        local.unwrap_or_default().to_owned(),
                        value.clone(),
                    ));
                },
                Some(
                    "intersectionOf" |
                    "unionOf" |
                    "complementOf" |
                    "oneOf" |
                    "inverseOf" |
                    "datatypeComplementOf" |
                    "onDatatype" |
                    "withRestrictions",
                ) => {
                    ensure!(
                        construct.is_none() ||
                            matches!(
                                (
                                    construct.as_ref().map(|(name, _)| *name),
                                    local
                                ),
                                (Some("onDatatype"), Some("withRestrictions")) |
                                    (
                                        Some("withRestrictions"),
                                        Some("onDatatype")
                                    )
                            ),
                        "an OWL expression needs exactly one constructor"
                    );
                    if local == Some("withRestrictions") {
                        restriction.facets = Some(value.clone());
                    } else if local == Some("onDatatype") {
                        restriction.datatype = Some(value.clone());
                    }
                    construct =
                        Some((local.unwrap_or_default(), value.clone()));
                },
                _ => bail!("unsupported OWL expression construct {predicate}"),
            }
        }
        if let Some((form, value)) = restriction.form.take() {
            ensure!(
                construct.is_none(),
                "an OWL restriction has no constructor"
            );
            return self.restriction(&restriction, &form, &value, depth).await;
        }
        if let Some(datatype) = restriction.datatype.take() {
            return self
                .datatype_restriction(
                    &datatype,
                    restriction.facets.take(),
                    depth,
                )
                .await;
        }
        let Some((name, value)) = construct else {
            bail!("an anonymous OWL expression has no constructor")
        };
        match name {
            "intersectionOf" | "unionOf" => {
                let operator = if name == "intersectionOf" {
                    " ⊓ "
                } else {
                    " ⊔ "
                };
                let mut parts = Vec::new(); // bounded: rdf_list::MEMBERS.
                for member in RdfList::new(self.view, value).members().await? {
                    parts.push(self.boxed(&member, depth + 1).await?.nested());
                }
                ensure!(
                    parts.len() >= 2,
                    "an OWL {name} needs at least two members"
                );
                Ok(Rendered::compound(parts.join(operator)))
            },
            "complementOf" | "datatypeComplementOf" => {
                Ok(Rendered::atom(format!(
                    "¬ {}",
                    self.boxed(&value, depth + 1).await?.nested()
                )))
            },
            "oneOf" => {
                let mut parts = Vec::new(); // bounded: rdf_list::MEMBERS.
                for member in RdfList::new(self.view, value).members().await? {
                    parts.push(self.boxed(&member, depth + 1).await?.text);
                }
                Ok(Rendered::atom(format!(
                    "oneOf ({})",
                    parts.join(", ")
                )))
            },
            "inverseOf" => {
                Ok(Rendered::atom(format!(
                    "{}⁻",
                    self.boxed(&value, depth + 1).await?.nested()
                )))
            },
            _ => bail!("unsupported OWL expression constructor {name}"),
        }
    }

    /// A property chain, `R ∘ S`.
    pub async fn chain(&self, list: Term) -> Result<String> {
        let mut parts = Vec::new(); // bounded: rdf_list::MEMBERS.
        for member in RdfList::new(self.view, list).members().await? {
            parts.push(self.boxed(&member, 1).await?.nested());
        }
        ensure!(
            parts.len() >= 2,
            "a property chain needs at least two properties"
        );
        Ok(parts.join(" ∘ "))
    }

    /// The names of a list of named resources, such as a key or a disjoint
    /// union.
    pub async fn named_list(&self, list: Term) -> Result<Vec<String>> {
        let mut parts = Vec::new(); // bounded: rdf_list::MEMBERS.
        for member in RdfList::new(self.view, list).members().await? {
            parts.push(self.boxed(&member, 1).await?.nested());
        }
        Ok(parts)
    }
}
