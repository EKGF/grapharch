//! One documented term as a table row: its name with labels, its source
//! annotations, and its axioms.
use {
    super::{
        annotations::{self, AnnotationLabels},
        axioms::{self, Axioms},
        expression::Expressions,
        kind::TermKind,
        names::TermNames,
    },
    crate::publication::{
        RDF_TYPE,
        dataset::Dataset,
        document::{
            annotated_term::escape,
            anonymous_order::AnonymousOrder,
            description::{MARKDOWN, SOURCE_DESCRIPTIONS},
        },
        language_range::{LanguageRange, LanguageTag},
        view::RevisionView,
    },
    anyhow::{Context, Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};

const LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
const PREFERRED: &str = "http://www.w3.org/2004/02/skos/core#prefLabel";
const DEFINED_BY: &str = "http://www.w3.org/2000/01/rdf-schema#isDefinedBy";
/// The most source statements one row renders.
const FACTS: usize = 512;

/// One labelled or plain entry of a row cell.
pub(super) struct Entry {
    label: Option<String>,
    value: String,
}

impl Entry {
    pub fn labelled(label: impl Into<String>, value: String) -> Self {
        Self { label: Some(label.into()), value }
    }

    pub fn plain(value: String) -> Self { Self { label: None, value } }

    /// One Markdown paragraph; the label is emphasized.
    pub fn markdown(&self) -> String {
        match &self.label {
            Some(label) => format!("*{}:* {}", escape(label), self.value),
            None => self.value.clone(),
        }
    }
}

/// What every row of one ontology's table reads its statements from.
pub(super) struct RowContext<'a> {
    pub dataset:     &'a Dataset,
    /// The ontology's own source document.
    pub source:      &'a RevisionView,
    /// The collected family, with the publication definitions, for labels
    /// of annotation properties and declarations of properties.
    pub collected:   &'a RevisionView,
    pub names:       &'a TermNames,
    /// The configured annotation properties in display order, or every
    /// annotation when absent.
    pub annotations: Option<&'a [NamedNode]>,
}

/// Declared object and datatype properties, whose statements about a term
/// are assertions rather than annotations.
#[derive(Default)]
pub(super) struct PropertyKinds {
    // bounded: one entry per distinct predicate within row fact budgets.
    assertions: BTreeMap<NamedNode, bool>,
}

impl PropertyKinds {
    async fn is_assertion(
        &mut self,
        dataset: &Dataset,
        view: &RevisionView,
        predicate: &NamedNode,
    ) -> Result<bool> {
        if let Some(known) = self.assertions.get(predicate) {
            return Ok(*known);
        }
        let pattern =
            view.pattern(&format!("{predicate} <{RDF_TYPE}> ?kind"), 0);
        let mut rows = dataset
            .select(format!(
                "SELECT ?kind WHERE {{ VALUES ?kind {{ \
                 <http://www.w3.org/2002/07/owl#ObjectProperty> \
                 <http://www.w3.org/2002/07/owl#DatatypeProperty> }} {pattern} }} \
                 LIMIT 1"
            ))
            .await?;
        let known = rows.next().await?.is_some();
        self.assertions.insert(predicate.clone(), known);
        Ok(known)
    }
}

/// One rendered term row.
pub(super) struct TermRow {
    pub subject:            NamedNode,
    pub kind:               TermKind,
    pub types:              Vec<NamedNode>,
    name:                   String,
    annotations:            Vec<Entry>,
    axioms:                 Vec<Entry>,
    pub missing_label:      bool,
    pub missing_definition: bool,
}

impl TermRow {
    /// Read one term's statements from its source document.
    pub async fn read(
        context: &RowContext<'_>,
        labels: &mut AnnotationLabels,
        properties: &mut PropertyKinds,
        subject: &NamedNode,
        anchor: Option<String>,
    ) -> Result<Self> {
        let facts = Self::facts(context, subject).await?;
        let types: Vec<NamedNode> = facts
            .iter()
            .filter(|(predicate, _)| predicate.as_str() == RDF_TYPE)
            .filter_map(|(_, value)| {
                match value {
                    Term::NamedNode(kind) => Some(kind.clone()),
                    _ => None,
                }
            })
            .collect();
        let kind = TermKind::of(&types).ok_or_else(|| {
            anyhow::anyhow!(
                "{subject} is not a declared class, datatype, property or \
                 named individual; a term table cannot document it"
            )
        })?;
        let expressions =
            Expressions::new(context.dataset, context.source, context.names);
        let mut axioms = Axioms::new(&expressions, context.names, kind);
        let english = LanguageRange::english();
        let mut names = Vec::new(); // bounded: FACTS.
        let mut annotations = Vec::new(); // bounded: FACTS.
        let mut missing_definition = true;
        let mut missing_label = true;
        for (predicate, value) in &facts {
            if let Term::Literal(literal) = value &&
                english.selects(literal)
            {
                if matches!(predicate.as_str(), LABEL | PREFERRED) {
                    missing_label = false;
                }
                if predicate.as_str() == MARKDOWN ||
                    SOURCE_DESCRIPTIONS.contains(&predicate.as_str())
                {
                    missing_definition = false;
                }
            }
            if predicate.as_str() == RDF_TYPE {
                let Term::NamedNode(declared) = value else {
                    bail!("{subject} rdf:type must be an IRI")
                };
                if !TermKind::is_declaration(declared) {
                    axioms.typed(value).await?;
                }
                continue;
            }
            if predicate.as_str() == DEFINED_BY {
                continue;
            }
            if predicate.as_str() == LABEL {
                let Term::Literal(label) = value else {
                    bail!("{subject} rdfs:label must be a literal")
                };
                if english.selects(label) {
                    names.push((
                        label.language().map(str::to_owned),
                        label.value().to_owned(),
                    ));
                }
                continue;
            }
            if axioms::is_axiom(predicate) {
                axioms.add(predicate, value).await.with_context(|| {
                    format!("rendering {predicate} of {subject}")
                })?;
                continue;
            }
            if properties
                .is_assertion(context.dataset, context.collected, predicate)
                .await?
            {
                axioms.assertion(predicate, value).await?;
                continue;
            }
            let position = match context.annotations {
                None => None,
                Some(selected) => {
                    match selected.iter().position(|p| p == predicate) {
                        Some(position) => Some(position),
                        None => continue,
                    }
                },
            };
            if let Term::Literal(literal) = value &&
                !english.selects(literal)
            {
                continue;
            }
            let entry = annotations::entry(
                labels,
                context.collected,
                context.names,
                predicate,
                value,
            )
            .await?;
            let label = labels.label(context.collected, predicate).await?;
            annotations.push(((position, label), entry));
        }
        annotations.sort_by(|a, b| a.0.cmp(&b.0));
        names.sort();
        names.dedup();
        Ok(Self {
            name: Self::name(context.names, subject, &names, anchor),
            subject: subject.clone(),
            kind,
            types,
            annotations: annotations.into_iter().map(|(_, e)| e).collect(),
            axioms: axioms.entries(),
            missing_label,
            missing_definition,
        })
    }

    async fn facts(
        context: &RowContext<'_>,
        subject: &NamedNode,
    ) -> Result<Vec<(NamedNode, Term)>> {
        let mut rows = context.source.statements(subject).await?;
        let mut facts = Vec::new(); // bounded: FACTS.
        while let Some(row) = rows.next().await? {
            ensure!(
                facts.len() < FACTS,
                "{subject} exceeds the {FACTS}-statement term budget"
            );
            let Term::NamedNode(predicate) = &row["predicate"] else {
                bail!("term predicate must be an IRI")
            };
            facts.push((predicate.clone(), row["value"].clone()));
        }
        AnonymousOrder::new(context.dataset, context.source)
            .arrange(facts)
            .await
            .with_context(|| format!("ordering facts of {subject}"))
    }

    /// The bold name, then every English label with its language tag.
    fn name(
        names: &TermNames,
        subject: &NamedNode,
        labels: &[(Option<String>, String)],
        anchor: Option<String>,
    ) -> String {
        let mut name = format!("**{}**", escape(&names.name(subject)));
        if !labels.is_empty() {
            name.push_str(&format!(
                " ({})",
                labels
                    .iter()
                    .map(|(tag, label)| {
                        match tag {
                            Some(tag) => {
                                format!(
                                    "{}@{}",
                                    escape(label),
                                    LanguageTag::new(tag).as_str()
                                )
                            },
                            None => escape(label),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        // The destination follows the text: a zero-width anchor that leads
        // a line still sets a word space.
        name.push_str(&anchor.unwrap_or_default());
        name
    }

    /// The Markdown of the name, annotations and axioms cells.
    pub fn cells(&self) -> [String; 3] {
        let join = |entries: &[Entry]| {
            entries
                .iter()
                .map(Entry::markdown)
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        [
            self.name.clone(),
            join(&self.annotations),
            join(&self.axioms),
        ]
    }
}
