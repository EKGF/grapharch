//! Bounded typed evidence loaded from the pinned plan, never diagnostic prose.
use {
    super::Diagnostic,
    crate::publication::{
        dataset::{Dataset, identity},
        term,
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Term},
    std::collections::BTreeMap,
};

#[derive(Default)]
pub(crate) struct EvidenceIndex(BTreeMap<NamedNode, Vec<Evidence>>);
pub(super) struct Evidence {
    pub predicate: NamedNode,
    pub value:     Literal,
    pub source:    Term,
}

pub(super) fn key(issue: &Diagnostic) -> NamedNode {
    // Identity only: the human diagnostic message is never parsed as evidence.
    identity("quality-presentation", &[
        &issue.code,
        &issue.reference,
        &issue.message,
    ])
}

impl EvidenceIndex {
    pub(super) fn get(&self, issue: &Diagnostic) -> &[Evidence] {
        self.0
            .get(&key(issue))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(super) async fn read(
        dataset: &Dataset,
        view: &RevisionView,
        plan: &NamedNode,
        identities: &BTreeMap<NamedNode, NamedNode>,
    ) -> Result<Self> {
        if identities.is_empty() {
            return Ok(Self::default());
        }
        let links = view.pattern(
            &format!("{plan} {} ?issue", term("diagnostic")),
            0,
        );
        let derived = view.pattern(
            "?issue <http://www.w3.org/ns/prov#wasDerivedFrom> ?statement",
            1,
        );
        let statement = view.pattern("?statement a <http://www.w3.org/1999/02/22-rdf-syntax-ns#Statement> ; <http://www.w3.org/1999/02/22-rdf-syntax-ns#predicate> ?predicate ; <http://www.w3.org/1999/02/22-rdf-syntax-ns#object> ?value ; <http://purl.org/dc/terms/source> ?source", 2);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?issue ?predicate ?value ?source WHERE {{ \
                 {links} {derived} {statement} }} ORDER BY ?issue ?predicate \
                 ?value ?source LIMIT 131073"
            ))
            .await?;
        let mut result = Self::default(); // bounded: 1024 findings, 128 records each, 16 MiB total lexical content.
        let mut count = 0;
        let mut bytes = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= 131072,
                "quality evidence record budget exceeded"
            );
            let (
                Term::NamedNode(issue),
                Term::NamedNode(predicate),
                Term::Literal(value),
            ) = (&row["issue"], &row["predicate"], &row["value"])
            else {
                anyhow::bail!("invalid quality evidence statement")
            };
            let identity = identities.get(issue).ok_or_else(|| {
                anyhow::anyhow!("evidence has unknown diagnostic")
            })?;
            let source = row["source"].clone();
            ensure!(
                matches!(source, Term::NamedNode(_) | Term::Literal(_)),
                "quality source must be an IRI or literal"
            );
            bytes += value.value().len() +
                predicate.as_str().len() +
                source.to_string().len();
            ensure!(
                bytes <= 16 * 1024 * 1024,
                "quality evidence exceeds 16 MiB budget"
            );
            let entries = result.0.entry(identity.clone()).or_default();
            ensure!(
                entries.len() < 128,
                "quality finding exceeds 128 evidence records"
            );
            entries.push(Evidence {
                predicate: predicate.clone(),
                value: value.clone(),
                source,
            });
        }
        Ok(result)
    }
}

impl Evidence {
    pub(super) fn field(&self) -> &str {
        match self.predicate.as_str() {
            "http://www.w3.org/2000/01/rdf-schema#label" => "Label",
            "http://www.w3.org/2004/02/skos/core#prefLabel" => {
                "Preferred label"
            },
            "http://purl.org/dc/terms/title" => "Title",
            "http://purl.org/dc/terms/description" => "Description",
            "http://www.w3.org/2004/02/skos/core#definition" => "Definition",
            "https://ekgf.org/ontology/annotation#markdownDescription" => {
                "Explanatory documentation"
            },
            _ => self.predicate.as_str(),
        }
    }

    pub(super) fn language(&self) -> String {
        match self.value.language() {
            Some("en") => "English (en)".into(),
            Some(language) => format!("Language: {language}"),
            None => "No language tag".into(),
        }
    }

    pub(super) fn source_text(&self) -> &str {
        match &self.source {
            Term::NamedNode(node) => node.as_str(),
            Term::Literal(value) => value.value(),
            _ => unreachable!("collector source must be an IRI or literal"),
        }
    }
}
