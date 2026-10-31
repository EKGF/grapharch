//! Source-owned terminology for a published collection of ontologies.
use {
    crate::publication::view::RevisionView,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};

pub(crate) struct Terminology {
    term: String,
}

impl Terminology {
    pub(crate) async fn read(
        view: &RevisionView,
        profile: &NamedNode,
    ) -> Result<Self> {
        let predicate = NamedNode::new_unchecked(
            "https://ekgf.org/ontology/artifact-publication#publishesFamily",
        );
        let Some(collection) = view.optional_by(profile, &predicate).await?
        else {
            return Self::new("family");
        };
        let Term::NamedNode(collection) = collection else {
            anyhow::bail!("publication collection must have an IRI")
        };
        let Some(value) = view.optional(&collection, "collectionTerm").await?
        else {
            return Self::new("family");
        };
        let Term::Literal(value) = value else {
            anyhow::bail!("collectionTerm must be a plain string")
        };
        ensure!(
            value.datatype().as_str() ==
                "http://www.w3.org/2001/XMLSchema#string",
            "collectionTerm must be a plain string"
        );
        Self::new(value.value())
    }

    fn new(term: &str) -> Result<Self> {
        ensure!(
            term.chars().next().is_some_and(char::is_alphanumeric) &&
                term.chars().last().is_some_and(char::is_alphanumeric) &&
                term.chars().count() <= 80 &&
                term.trim() == term &&
                !term.contains("  ") &&
                term.chars().all(|c| {
                    c.is_alphanumeric() || matches!(c, ' ' | '-' | '\'')
                }),
            "collectionTerm requires a plain noun phrase of 1–80 characters"
        );
        Ok(Self { term: term.into() })
    }

    pub(crate) fn singular(&self) -> &str { &self.term }

    pub(crate) fn title(&self) -> String {
        let mut chars = self.term.chars();
        let first = chars.next().expect("validated nonempty collection term");
        first.to_uppercase().chain(chars).collect()
    }

    pub(crate) fn overview(&self) -> String {
        format!("{} overview", self.title())
    }
}

#[cfg(test)]
mod tests {
    use super::Terminology;

    #[test]
    fn collection_terms_are_bounded_plain_noun_phrases() {
        for term in [
            "library",
            "ontology suite",
            "portfolio",
            "éditeur's library",
        ] {
            assert!(Terminology::new(term).is_ok(), "{term}");
        }
        for term in [
            "",
            " ",
            " library",
            "library ",
            "library\n",
            "library  suite",
            "<library>",
            "#library",
            "'",
            "-library",
            "library-",
        ] {
            assert!(Terminology::new(term).is_err(), "{term:?}");
        }
        assert!(Terminology::new(&"a".repeat(81)).is_err());
    }
}
