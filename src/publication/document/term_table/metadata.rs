//! The metadata table of one ontology: its IRI and its header statements,
//! one row per value.
use {
    super::{markdown::TableMarkdown, names::TermNames},
    crate::publication::{
        RDF_TYPE,
        document::{anonymous_order::AnonymousOrder, plain_text::PlainText},
        language_range::LanguageRange,
        view::RevisionView,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};

/// The most ontology header statements one table shows.
const STATEMENTS: usize = 512;

/// The header statements of one ontology, grouped by predicate.
pub(super) struct OntologyMetadata {
    ontology:   NamedNode,
    // bounded: STATEMENTS.
    statements: BTreeMap<NamedNode, Vec<Term>>,
}

impl OntologyMetadata {
    pub async fn read(
        view: &RevisionView,
        ontology: &NamedNode,
    ) -> Result<Self> {
        let mut rows = view.statements(ontology).await?;
        let mut statements: BTreeMap<NamedNode, Vec<Term>> = BTreeMap::new();
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= STATEMENTS,
                "{ontology} exceeds the {STATEMENTS}-statement metadata budget"
            );
            let Term::NamedNode(predicate) = &row["predicate"] else {
                bail!("ontology predicate must be an IRI")
            };
            if predicate.as_str() == RDF_TYPE {
                continue;
            }
            statements
                .entry(predicate.clone())
                .or_default()
                .push(row["value"].clone());
        }
        Ok(Self { ontology: ontology.clone(), statements })
    }

    /// The configured predicates in order, or every predicate by its
    /// prefixed name.
    fn predicates(
        &self,
        names: &TermNames,
        selected: Option<&[NamedNode]>,
    ) -> Vec<NamedNode> {
        match selected {
            Some(selected) => selected.to_vec(),
            None => {
                let mut predicates: Vec<_> =
                    self.statements.keys().cloned().collect();
                predicates
                    .sort_by_cached_key(|predicate| names.curie(predicate));
                predicates
            },
        }
    }

    fn value(names: &TermNames, value: &Term) -> Result<String> {
        Ok(match value {
            // Header values keep their exact lexical form, dates included.
            Term::Literal(literal) => PlainText::new(literal.value()).lines(),
            Term::NamedNode(_) if AnonymousOrder::is_anonymous(value) => {
                bail!(
                    "anonymous ontology metadata values are not supported in \
                     a metadata table"
                )
            },
            Term::NamedNode(node) => names.value(node),
            _ => bail!("unsupported ontology metadata value"),
        })
    }

    /// Append the two-column table rows.
    pub fn write(
        &self,
        table: &mut TableMarkdown,
        names: &TermNames,
        selected: Option<&[NamedNode]>,
    ) -> Result<()> {
        let english = LanguageRange::english();
        table.table(&[("Metadata Term", 30), ("Value", 70)])?;
        table.cell("Ontology IRI")?;
        table.cell(&names.value(&self.ontology))?;
        for predicate in self.predicates(names, selected) {
            let Some(values) = self.statements.get(&predicate) else {
                continue;
            };
            let mut rendered = Vec::new(); // bounded: STATEMENTS.
            for value in values {
                if let Term::Literal(literal) = value &&
                    !english.selects(literal)
                {
                    continue;
                }
                rendered.push(Self::value(names, value)?);
            }
            rendered.sort();
            for value in rendered {
                table.cell(&format!("`{}`", names.curie(&predicate)))?;
                table.cell(&value)?;
            }
        }
        table.end()
    }
}
