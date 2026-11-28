//! The terms of the GraphArch namespace this engine declares. Publication
//! definitions may use only those: an undeclared term means the definitions
//! need a newer GraphArch, or contain a mistake, and silently ignoring it
//! would build as if the statement were absent.
use {
    super::{NAMESPACE, RDF_TYPE, dataset::CONTROL, term, view::RevisionView},
    anyhow::{Result, bail},
    oxrdf::{NamedOrBlankNode, Term},
    oxrdfio::{RdfFormat, RdfParser},
    std::{collections::HashSet, fmt, sync::LazyLock},
};

/// The vocabulary, shapes and catalogues that declare GraphArch terms.
const DECLARATIONS: [&str; 15] = [
    include_str!("../../docs/other/ontology/publication.ttl"),
    include_str!("../../docs/other/ontology/publication.shacl.ttl"),
    include_str!("../../docs/other/ontology/collectors.ttl"),
    include_str!("../../docs/other/ontology/conformance-languages.ttl"),
    include_str!("../../docs/other/ontology/doc-component-categories.ttl"),
    include_str!("../../docs/other/ontology/document-types.ttl"),
    include_str!("../../docs/other/ontology/documentation.ttl"),
    include_str!("../../docs/other/ontology/documentors.ttl"),
    include_str!("../../docs/other/ontology/rdf-formats.ttl"),
    include_str!("../../docs/other/ontology/repository.ttl"),
    include_str!("../../docs/other/ontology/renderers.ttl"),
    include_str!("../../docs/other/ontology/showcase-runners.ttl"),
    include_str!("../../docs/other/ontology/themes.ttl"),
    include_str!("../../docs/other/ontology/mkdocs-import.ttl"),
    include_str!("../../docs/other/ontology/website-presentation.ttl"),
];

// bounded: the compile-time vocabulary, independent of any corpus.
static DECLARED: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut declared = HashSet::new();
    for source in DECLARATIONS {
        for quad in RdfParser::from_format(RdfFormat::Turtle)
            .for_slice(source.as_bytes())
        {
            let quad = quad.expect("shipped vocabulary is valid Turtle");
            if let NamedOrBlankNode::NamedNode(subject) = quad.subject &&
                subject.as_str().starts_with(NAMESPACE)
            {
                declared.insert(subject.into_string());
            }
        }
    }
    declared
});

/// How a statement uses a GraphArch term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TermRole {
    /// The term is the predicate of a statement.
    Predicate,
    /// The term is the class of an `rdf:type` statement.
    Class,
    /// The term is the value of a GraphArch predicate.
    Individual,
}

impl TermRole {
    /// Every role, in the order a definition is checked.
    pub(crate) const ALL: [Self; 3] =
        [Self::Predicate, Self::Class, Self::Individual];

    fn name(self) -> &'static str {
        match self {
            Self::Predicate => "predicate",
            Self::Class => "class",
            Self::Individual => "individual",
        }
    }

    /// SPARQL pattern binding `?term` in this role.
    pub(crate) fn pattern(self) -> String {
        match self {
            Self::Predicate => "?subject ?term ?object".to_owned(),
            Self::Class => format!("?subject <{RDF_TYPE}> ?term"),
            Self::Individual => {
                format!(
                    "?subject ?predicate ?term FILTER(isIRI(?term) && \
                     STRSTARTS(STR(?predicate), \"{NAMESPACE}\") && \
                     ?predicate != <{RDF_TYPE}>)"
                )
            },
        }
    }
}

/// A GraphArch term that this release does not declare, with where
/// definitions used it.
#[derive(Debug)]
pub(crate) struct UnknownTerm {
    term:   String,
    role:   TermRole,
    source: String,
}

impl UnknownTerm {
    pub(crate) fn new(term: &str, role: TermRole, source: &str) -> Self {
        Self {
            term: term.to_owned(),
            role,
            source: source.to_owned(),
        }
    }
}

impl fmt::Display for UnknownTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let local = self.term.strip_prefix(NAMESPACE).unwrap_or(&self.term);
        write!(
            f,
            "definitions in {} use ga:{local} ({}) as a {}, which GraphArch \
             {} does not declare; this term may need a newer GraphArch \
             release, or the definitions contain a mistake",
            self.source,
            self.term,
            self.role.name(),
            env!("CARGO_PKG_VERSION")
        )
    }
}

impl std::error::Error for UnknownTerm {}

/// The GraphArch namespace as this release declares it.
pub(crate) struct Vocabulary;

impl Vocabulary {
    /// Whether `iri` belongs to the GraphArch namespace.
    pub(crate) fn owns(iri: &str) -> bool { iri.starts_with(NAMESPACE) }

    /// Whether this release declares `iri`.
    pub(crate) fn declares(iri: &str) -> bool { DECLARED.contains(iri) }

    /// Reject `iri` when it is a GraphArch term this release does not declare.
    pub(crate) fn require(
        iri: &str,
        role: TermRole,
        source: &str,
    ) -> Result<()> {
        if Self::owns(iri) && !Self::declares(iri) {
            bail!(UnknownTerm::new(iri, role, source))
        }
        Ok(())
    }

    /// Reject every undeclared GraphArch term a definitions store uses;
    /// `source` names the file the definitions were read from.
    pub(crate) fn require_store(
        store: &oxigraph::store::Store,
        source: &str,
    ) -> Result<()> {
        for quad in store.iter() {
            let quad = quad?;
            Self::require(
                quad.predicate.as_str(),
                TermRole::Predicate,
                source,
            )?;
            let ga_predicate = Self::owns(quad.predicate.as_str());
            if let Term::NamedNode(object) = &quad.object {
                if quad.predicate.as_str() == RDF_TYPE {
                    Self::require(object.as_str(), TermRole::Class, source)?;
                } else if ga_predicate {
                    Self::require(
                        object.as_str(),
                        TermRole::Individual,
                        source,
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl RevisionView {
    /// Reject the first undeclared GraphArch term this revision uses as a
    /// predicate, class or individual. A mention of such an IRI as the value
    /// of a foreign predicate is content and passes.
    pub(crate) async fn require_declared_terms(&self) -> Result<()> {
        for role in TermRole::ALL {
            let pattern = self.pattern(&role.pattern(), 0);
            let mut rows = self
                .dataset()
                .select(format!(
                    "SELECT DISTINCT ?term ?origin WHERE {{ {pattern} GRAPH \
                     <{CONTROL}> {{ ?_attempt0 {} ?origin }} \
                     FILTER(isIRI(?term) && STRSTARTS(STR(?term), \
                     \"{NAMESPACE}\")) }}",
                    term("source")
                ))
                .await?;
            while let Some(row) = rows.next().await? {
                let Term::NamedNode(term) = &row["term"] else {
                    continue
                };
                let source = match row.get("origin") {
                    Some(Term::Literal(origin)) => origin.value().to_owned(),
                    _ => "the project definitions".to_owned(),
                };
                Vocabulary::require(term.as_str(), role, &source)?;
            }
        }
        Ok(())
    }
}
