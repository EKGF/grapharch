//! A retained SHACL query is captioned by the text of the resource that owns
//! it: its `sh:message`, else its `sh:description`, else its `rdfs:comment`.
use {
    super::annotated_term::escape,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, NamedNodeRef, Term},
};

const SH_MESSAGE: NamedNodeRef<'_> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#message");
const SH_DESCRIPTION: NamedNodeRef<'_> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#description");

/// Annotations that caption query code, in order of precedence.
#[derive(Clone, Copy)]
enum CaptionSource {
    Message,
    Description,
    Comment,
}

impl CaptionSource {
    const ORDER: [Self; 3] = [Self::Message, Self::Description, Self::Comment];

    fn of(predicate: &NamedNode) -> Option<Self> {
        let predicate = predicate.as_ref();
        if predicate == SH_MESSAGE {
            Some(Self::Message)
        } else if predicate == SH_DESCRIPTION {
            Some(Self::Description)
        } else if predicate == oxrdf::vocab::rdfs::COMMENT {
            Some(Self::Comment)
        } else {
            None
        }
    }

    fn index(self) -> usize { self as usize }
}

/// Caption text gathered from one query owner in the publication language.
#[derive(Default)]
pub(super) struct QueryCaption {
    // bounded: the owner's facts are capped at 512 by the term documentor.
    values: [Vec<String>; 3],
}

impl QueryCaption {
    /// Gather the caption from an anonymous resource's retained facts.
    pub fn from_facts(
        facts: &[(NamedNode, Term)],
        language: &str,
    ) -> Result<Self> {
        let mut caption = Self::default();
        for (predicate, value) in facts {
            caption.observe(predicate, value, language)?;
        }
        Ok(caption)
    }

    /// Record one fact of the owner when it is a caption annotation.
    pub fn observe(
        &mut self,
        predicate: &NamedNode,
        value: &Term,
        language: &str,
    ) -> Result<()> {
        let Some(source) = CaptionSource::of(predicate) else {
            return Ok(())
        };
        let Term::Literal(literal) = value else {
            anyhow::bail!("SHACL query caption {predicate} must be a literal")
        };
        ensure!(
            !literal.value().trim().is_empty(),
            "SHACL query caption {predicate} cannot be empty"
        );
        if super::description::selected(literal, language) {
            let values = &mut self.values[source.index()];
            if !values.iter().any(|known| known == literal.value()) {
                values.push(literal.value().to_owned());
            }
        }
        Ok(())
    }

    /// Markdown caption, or `None` when the owner carries no caption text.
    pub fn markdown(&self) -> Option<String> {
        CaptionSource::ORDER.iter().find_map(|source| {
            let values = &self.values[source.index()];
            (!values.is_empty()).then(|| {
                let mut values = values.clone();
                values.sort();
                escape(&values.join("; "))
            })
        })
    }
}
