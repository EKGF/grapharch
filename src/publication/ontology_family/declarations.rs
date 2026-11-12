//! A family member's `sh:declare` bindings: its own namespace plus any further
//! display prefixes the source declares for vocabularies it reuses.
use {
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};

/// Maximum `sh:declare` bindings read for one member ontology.
const MEMBER_DECLARATIONS: usize = 64;

/// One source `sh:declare` binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Declaration {
    pub(super) prefix:    String,
    pub(super) namespace: String,
}

/// The declarations of one ontology, split into its own namespace and the
/// additional display aliases.
pub(super) struct MemberDeclarations {
    pub(super) own:     Declaration,
    pub(super) aliases: Vec<Declaration>,
}

impl MemberDeclarations {
    /// Read the ontology's bindings. With one declaration, that declaration
    /// names the ontology's namespace. With several, exactly one must declare
    /// the ontology IRI itself as its namespace; the others are aliases.
    pub(super) async fn read(
        dataset: &Dataset,
        view: &RevisionView,
        ontology: &NamedNode,
    ) -> Result<Self> {
        let pattern = view.pattern(
            &format!(
                "{ontology} <http://www.w3.org/ns/shacl#declare> ?declaration \
                 . ?declaration <http://www.w3.org/ns/shacl#prefix> ?prefix ; \
                 <http://www.w3.org/ns/shacl#namespace> ?namespace"
            ),
            0,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?prefix ?namespace WHERE {{ {pattern} }} \
                 ORDER BY ?namespace ?prefix LIMIT {}",
                MEMBER_DECLARATIONS + 1
            ))
            .await?;
        let mut declarations = Vec::new(); // bounded: MEMBER_DECLARATIONS.
        while let Some(row) = rows.next().await? {
            ensure!(
                declarations.len() < MEMBER_DECLARATIONS,
                "ontology {ontology} exceeds {MEMBER_DECLARATIONS} namespace \
                 declarations"
            );
            let (Term::Literal(prefix), Term::Literal(namespace)) =
                (&row["prefix"], &row["namespace"])
            else {
                anyhow::bail!("namespace metadata must be literal")
            };
            ensure!(
                !prefix.value().is_empty() && !namespace.value().is_empty(),
                "namespace metadata cannot be empty"
            );
            declarations.push(Declaration {
                prefix:    prefix.value().to_owned(),
                namespace: namespace.value().to_owned(),
            });
        }
        Self::split(ontology, declarations)
    }

    fn split(
        ontology: &NamedNode,
        mut declarations: Vec<Declaration>,
    ) -> Result<Self> {
        ensure!(
            !declarations.is_empty(),
            "ontology {ontology} requires explicit sh:declare namespace \
             metadata"
        );
        if declarations.len() == 1 {
            return Ok(Self {
                own:     declarations.remove(0),
                aliases: Vec::new(),
            })
        }
        let mut own = declarations
            .iter()
            .enumerate()
            .filter(|(_, d)| d.namespace == ontology.as_str());
        let (index, _) = own.next().ok_or_else(|| {
            anyhow::anyhow!(
                "ontology namespace declaration is ambiguous: {ontology} has \
                 several sh:declare bindings and none declares the ontology \
                 IRI as its namespace"
            )
        })?;
        ensure!(
            own.next().is_none(),
            "ontology namespace declaration is ambiguous: {ontology} declares \
             several prefixes for its own namespace"
        );
        let own = declarations.remove(index);
        Ok(Self { own, aliases: declarations })
    }
}

#[cfg(test)]
mod tests {
    use super::{Declaration, MemberDeclarations};

    fn declaration(prefix: &str, namespace: &str) -> Declaration {
        Declaration {
            prefix:    prefix.into(),
            namespace: namespace.into(),
        }
    }

    #[test]
    fn own_namespace_is_the_ontology_iri_among_several() -> anyhow::Result<()> {
        let ontology = oxrdf::NamedNode::new("https://example.org/vocab#")?;
        let split = MemberDeclarations::split(&ontology, vec![
            declaration("orcid", "http://orcid.org/"),
            declaration("ex", "https://example.org/vocab#"),
        ])?;
        assert_eq!(
            split.own,
            declaration("ex", "https://example.org/vocab#")
        );
        assert_eq!(split.aliases, vec![declaration(
            "orcid",
            "http://orcid.org/"
        )]);
        Ok(())
    }

    #[test]
    fn several_declarations_without_the_ontology_namespace_fail()
    -> anyhow::Result<()> {
        let ontology = oxrdf::NamedNode::new("https://example.org/vocab#")?;
        assert!(
            MemberDeclarations::split(&ontology, vec![
                declaration("a", "https://example.org/a#"),
                declaration("b", "https://example.org/b#"),
            ])
            .is_err()
        );
        Ok(())
    }
}
