//! Display namespace bindings retained as source RDF, independent of parser
//! prefix spellings. These labels never change a resource's canonical IRI.
//!
//! Turtle `@prefix` directives are parser syntax: they are not retained in the
//! collected KG, so renderers cannot honour them. Sources that need a display
//! prefix beyond the well-known vocabularies declare it as RDF with
//! `sh:declare`.
use {
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet},
};

/// Prefixes of widely reused vocabularies, applied to every publication.
/// Source declarations outrank them.
const WELL_KNOWN: [(&str, &str); 21] = [
    (
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "rdf",
    ),
    ("http://www.w3.org/2000/01/rdf-schema#", "rdfs"),
    ("http://www.w3.org/2002/07/owl#", "owl"),
    ("http://www.w3.org/2001/XMLSchema#", "xsd"),
    ("http://www.w3.org/ns/shacl#", "sh"),
    ("http://www.w3.org/2004/02/skos/core#", "skos"),
    ("http://purl.org/dc/elements/1.1/", "dc"),
    ("http://purl.org/dc/terms/", "dct"),
    ("http://www.w3.org/ns/dcat#", "dcat"),
    ("http://www.w3.org/ns/prov#", "prov"),
    ("http://purl.org/vocab/vann/", "vann"),
    ("http://xmlns.com/foaf/0.1/", "foaf"),
    ("https://schema.org/", "schema"),
    ("http://www.w3.org/2006/time#", "time"),
    ("http://www.w3.org/ns/odrl/2/", "odrl"),
    ("http://www.w3.org/ns/org#", "org"),
    (
        "https://www.omg.org/spec/Commons/PartiesAndSituations/",
        "cmns-pts",
    ),
    (
        "https://www.omg.org/spec/Commons/RolesAndCompositions/",
        "cmns-rlcmp",
    ),
    (
        "https://www.omg.org/spec/Commons/Organizations/",
        "cmns-org",
    ),
    (
        "https://www.omg.org/spec/Commons/Classifiers/",
        "cmns-cls",
    ),
    (
        "https://www.omg.org/spec/Commons/Designators/",
        "cmns-dsg",
    ),
];

/// Maximum explicit source declarations read for one publication.
const DECLARATION_BUDGET: usize = 256;

/// Namespace-to-prefix display bindings with deterministic precedence:
/// source declarations outrank publication defaults, which outrank the
/// well-known vocabularies. Two source declarations of one prefix for
/// different namespaces are an error.
#[derive(Clone, Debug)]
pub(crate) struct NamespaceBindings {
    // bounded: well-known entries plus the source declaration budget.
    bindings: BTreeMap<String, String>,
    declared: BTreeSet<String>,
}

impl NamespaceBindings {
    /// Bindings for the well-known vocabularies only.
    pub(crate) fn well_known() -> Self {
        Self {
            bindings: WELL_KNOWN
                .iter()
                .map(|(namespace, prefix)| {
                    ((*namespace).to_owned(), (*prefix).to_owned())
                })
                .collect(),
            declared: BTreeSet::new(),
        }
    }

    /// Add a publication default. It replaces a well-known binding of the
    /// same namespace or prefix, never a source declaration: a default whose
    /// namespace or prefix is already declared is not applied.
    pub(crate) fn with_default(
        mut self,
        namespace: impl Into<String>,
        prefix: impl Into<String>,
    ) -> Self {
        let (namespace, prefix) = (namespace.into(), prefix.into());
        let claimed = self.declared.contains(&namespace) ||
            self.bindings.iter().any(|(existing, bound)| {
                bound == &prefix && self.declared.contains(existing)
            });
        if !claimed {
            self.release(&namespace, &prefix);
            self.bindings.insert(namespace, prefix);
        }
        self
    }

    /// Record a source declaration. It outranks any non-source binding of the
    /// same namespace or prefix.
    pub(crate) fn declare(
        &mut self,
        namespace: &str,
        prefix: &str,
    ) -> Result<()> {
        NamedNode::new(namespace)?;
        ensure!(
            !prefix.is_empty(),
            "source namespace prefix cannot be empty"
        );
        ensure!(
            !self.bindings.iter().any(|(existing, bound)| {
                bound == prefix &&
                    existing != namespace &&
                    self.declared.contains(existing)
            }),
            "namespace prefix {prefix} is declared for several namespaces"
        );
        ensure!(
            self.declared.len() < DECLARATION_BUDGET ||
                self.declared.contains(namespace),
            "source namespace declaration budget exceeded"
        );
        self.release(namespace, prefix);
        self.bindings
            .insert(namespace.to_owned(), prefix.to_owned());
        self.declared.insert(namespace.to_owned());
        Ok(())
    }

    /// Record a further alias unless its namespace is already declared, so an
    /// ontology's own declaration is never replaced by another's alias.
    pub(crate) fn declare_alias(
        &mut self,
        namespace: &str,
        prefix: &str,
    ) -> Result<()> {
        if self.declared.contains(namespace) {
            return Ok(())
        }
        self.declare(namespace, prefix)
    }

    /// Apply every `sh:declare` binding of the ontologies in a view.
    pub(crate) async fn read_declarations(
        mut self,
        dataset: &Dataset,
        view: &RevisionView,
    ) -> Result<Self> {
        let pattern = view.pattern(
            "?ontology a <http://www.w3.org/2002/07/owl#Ontology> ; \
             <http://www.w3.org/ns/shacl#declare> ?declaration . \
             ?declaration <http://www.w3.org/ns/shacl#prefix> ?prefix ; \
             <http://www.w3.org/ns/shacl#namespace> ?namespace",
            0,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?namespace ?prefix WHERE {{ {pattern} }} \
                 ORDER BY ?namespace ?prefix LIMIT {}",
                DECLARATION_BUDGET + 1
            ))
            .await?;
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= DECLARATION_BUDGET,
                "source namespace declaration budget exceeded"
            );
            let (Term::Literal(namespace), Term::Literal(prefix)) =
                (&row["namespace"], &row["prefix"])
            else {
                anyhow::bail!(
                    "sh:declare namespace and prefix must be literals"
                )
            };
            // Several local aliases for one namespace are legal. The rows are
            // ordered, so the first lexical alias wins deterministically.
            self.declare_alias(namespace.value(), prefix.value())?;
        }
        Ok(self)
    }

    /// The display map consumed by documentors: namespace to prefix.
    pub(crate) fn into_map(self) -> BTreeMap<String, String> { self.bindings }

    fn release(&mut self, namespace: &str, prefix: &str) {
        self.bindings
            .retain(|existing, bound| existing == namespace || bound != prefix);
    }
}

#[cfg(test)]
mod tests {
    use super::NamespaceBindings;

    #[test]
    fn well_known_vocabularies_include_dublin_core_elements() {
        let map = NamespaceBindings::well_known().into_map();
        for (namespace, prefix) in [
            ("http://purl.org/dc/elements/1.1/", "dc"),
            ("http://purl.org/dc/terms/", "dct"),
            ("http://www.w3.org/ns/dcat#", "dcat"),
            ("http://www.w3.org/ns/prov#", "prov"),
            ("http://purl.org/vocab/vann/", "vann"),
            ("http://xmlns.com/foaf/0.1/", "foaf"),
            ("https://schema.org/", "schema"),
            ("http://www.w3.org/2006/time#", "time"),
        ] {
            assert_eq!(
                map.get(namespace).map(String::as_str),
                Some(prefix)
            );
        }
        let prefixes: std::collections::BTreeSet<_> = map.values().collect();
        assert_eq!(
            prefixes.len(),
            map.len(),
            "prefixes must be unique"
        );
    }

    #[test]
    fn source_declarations_outrank_well_known_prefixes() -> anyhow::Result<()> {
        let mut bindings = NamespaceBindings::well_known();
        bindings.declare("https://example.org/time#", "time")?;
        bindings.declare("http://purl.org/dc/elements/1.1/", "dce")?;
        let map = bindings.into_map();
        assert_eq!(
            map.get("https://example.org/time#").map(String::as_str),
            Some("time")
        );
        assert!(!map.contains_key("http://www.w3.org/2006/time#"));
        assert_eq!(
            map.get("http://purl.org/dc/elements/1.1/")
                .map(String::as_str),
            Some("dce")
        );
        Ok(())
    }

    #[test]
    fn conflicting_source_declarations_fail() -> anyhow::Result<()> {
        let mut bindings = NamespaceBindings::well_known();
        bindings.declare("https://example.org/a#", "ex")?;
        assert!(bindings.declare("https://example.org/b#", "ex").is_err());
        Ok(())
    }

    #[test]
    fn aliases_never_replace_a_declared_namespace() -> anyhow::Result<()> {
        let mut bindings = NamespaceBindings::well_known();
        bindings.declare("https://example.org/a#", "own")?;
        bindings.declare_alias("https://example.org/a#", "alias")?;
        assert_eq!(
            bindings
                .into_map()
                .get("https://example.org/a#")
                .map(String::as_str),
            Some("own")
        );
        Ok(())
    }

    #[test]
    fn publication_defaults_never_replace_source_declarations()
    -> anyhow::Result<()> {
        let mut bindings = NamespaceBindings::well_known();
        bindings.declare("https://example.org/a#", "own")?;
        let map = bindings
            .with_default("https://example.org/a#", "fallback")
            .into_map();
        assert_eq!(
            map.get("https://example.org/a#").map(String::as_str),
            Some("own")
        );
        Ok(())
    }
}
