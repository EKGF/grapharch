//! Annotation entries of a term row, labelled by the annotation property's
//! own English label from the collected graph.
use {
    super::{names::TermNames, row::Entry},
    crate::publication::{
        document::{anonymous_order::AnonymousOrder, plain_text::PlainText},
        language_range::LanguageRange,
        view::RevisionView,
    },
    anyhow::{Result, bail},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};

const LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";

/// Display labels of annotation properties, read once per property.
#[derive(Default)]
pub(super) struct AnnotationLabels {
    // bounded: one entry per distinct annotation property of one ontology,
    // within its terms' fact budgets.
    labels: BTreeMap<NamedNode, String>,
}

impl AnnotationLabels {
    /// The property's English `rdfs:label`, with an initial capital. An
    /// untagged or exactly `en` label is preferred over regional variants.
    pub async fn label(
        &mut self,
        view: &RevisionView,
        property: &NamedNode,
    ) -> Result<String> {
        if let Some(label) = self.labels.get(property) {
            return Ok(label.clone());
        }
        let english = LanguageRange::english();
        let mut rows = view
            .objects_by(property, &NamedNode::new_unchecked(LABEL))
            .await?;
        let mut candidates = Vec::new(); // bounded: 64 labels.
        while let Some(row) = rows.next().await? {
            anyhow::ensure!(
                candidates.len() < 64,
                "{property} exceeds the 64-label budget"
            );
            let Term::Literal(label) = &row["value"] else {
                bail!("{property} rdfs:label must be a literal")
            };
            if english.selects(label) && !label.value().trim().is_empty() {
                let regional = label
                    .language()
                    .is_some_and(|tag| !tag.eq_ignore_ascii_case("en"));
                candidates.push((regional, label.value().trim().to_owned()));
            }
        }
        candidates.sort();
        let Some((_, label)) = candidates.into_iter().next() else {
            bail!(
                "annotation property {property} shown in a term table has no \
                 English rdfs:label in the collected sources or publication \
                 definitions"
            )
        };
        let mut characters = label.chars();
        let label = match characters.next() {
            Some(first) => first.to_uppercase().chain(characters).collect(),
            None => label,
        };
        self.labels.insert(property.clone(), label.clone());
        Ok(label)
    }
}

/// One annotation value as inert Markdown.
pub(super) fn value(names: &TermNames, value: &Term) -> Result<String> {
    Ok(match value {
        Term::Literal(literal) => PlainText::new(literal.value()).prose(),
        Term::NamedNode(_) if AnonymousOrder::is_anonymous(value) => {
            bail!(
                "anonymous annotation values are not supported in a term table"
            )
        },
        Term::NamedNode(node) => names.value(node),
        _ => bail!("unsupported annotation value"),
    })
}

/// The labelled entry of one annotation statement.
pub(super) async fn entry(
    labels: &mut AnnotationLabels,
    view: &RevisionView,
    names: &TermNames,
    property: &NamedNode,
    object: &Term,
) -> Result<Entry> {
    Ok(Entry::labelled(
        labels.label(view, property).await?,
        value(names, object)?,
    ))
}
