//! Download file names of PDF editions, derived only from definitions and an
//! explicit edition build input:
//! `<authority>-<document-type>-<document-name>[-<edition>].pdf`.
use {
    super::view::RevisionView,
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::fmt,
};

const PUBLICATION: &str = "https://ekgf.org/ontology/artifact-publication#";

/// Lowercase ASCII letters and digits in hyphen-separated words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slug(String);

impl Slug {
    /// Validate a slug without normalizing it.
    pub fn try_new(value: &str) -> Result<Self> {
        ensure!(
            !value.is_empty() &&
                value.len() <= 64 &&
                value.split('-').all(|word| {
                    !word.is_empty() &&
                        word.bytes().all(|byte| {
                            byte.is_ascii_lowercase() || byte.is_ascii_digit()
                        })
                }),
            "invalid slug {value:?}: use lowercase ASCII letters and digits \
             in hyphen-separated words, at most 64 characters"
        );
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str { &self.0 }

    async fn declared(
        view: &RevisionView,
        subject: &NamedNode,
    ) -> Result<Option<Self>> {
        match view.optional(subject, "slug").await? {
            None => Ok(None),
            Some(Term::Literal(value)) => {
                Ok(Some(Self::try_new(value.value())?))
            },
            Some(_) => bail!("{subject} ga:slug must be a literal"),
        }
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Which build of a publication produces its PDF edition. Only the build of
/// the declared default branch may leave the file name unsuffixed; every other
/// build names its branch, `pr-<number>` or release version, so builds never
/// overwrite each other's PDFs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edition {
    /// The build of the declared default branch.
    Default,
    /// A branch, pull request or version build.
    Named(Slug),
}

impl Edition {
    /// Parse a build input: `default`, or a slug such as `pr-23`.
    pub fn parse(value: &str) -> Result<Self> {
        if value == "default" {
            Ok(Self::Default)
        } else {
            Ok(Self::Named(Slug::try_new(value)?))
        }
    }
}

/// The kind of document a publication is, from the shipped catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentType {
    /// A normative specification.
    Specification,
    /// An overview or introduction.
    Overview,
    /// A formal submission to a standards body.
    Submission,
    /// A review edition.
    Review,
}

impl DocumentType {
    /// Resolve a catalogue entry by its exact IRI.
    pub fn from_node(value: &NamedNode) -> Result<Self> {
        match value.as_str() {
            "https://ekgf.org/ontology/grapharch#SpecificationDocument" => {
                Ok(Self::Specification)
            },
            "https://ekgf.org/ontology/grapharch#OverviewDocument" => {
                Ok(Self::Overview)
            },
            "https://ekgf.org/ontology/grapharch#SubmissionDocument" => {
                Ok(Self::Submission)
            },
            "https://ekgf.org/ontology/grapharch#ReviewDocument" => {
                Ok(Self::Review)
            },
            other => bail!("unsupported document type {other}"),
        }
    }

    /// File-name segment of this document type.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Specification => "spec",
            Self::Overview => "overview",
            Self::Submission => "submission",
            Self::Review => "review",
        }
    }
}

/// The resolved download name of one target's PDF edition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfName {
    authority: Slug,
    kind:      DocumentType,
    name:      Slug,
    edition:   Edition,
}

impl PdfName {
    /// Read the declared naming parts of `target`. Every part is required;
    /// nothing is derived from titles or file layout.
    pub async fn read(
        view: &RevisionView,
        target: &NamedNode,
        edition: Option<&Edition>,
    ) -> Result<Self> {
        let edition = edition.cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "PDF target {target} requires an explicit edition build \
                 input: --edition default for the declared default branch, \
                 otherwise the branch slug, pr-<number> or release version"
            )
        })?;
        let publisher = match view.optional(target, "publisher").await? {
            Some(Term::NamedNode(publisher)) => publisher,
            Some(_) => bail!("{target} ga:publisher must be an IRI"),
            None => {
                bail!(
                    "PDF target {target} requires a ga:publisher with a \
                     ga:slug for its file name"
                )
            },
        };
        let authority =
            Slug::declared(view, &publisher).await?.ok_or_else(|| {
                anyhow::anyhow!(
                    "publisher {publisher} of PDF target {target} requires a \
                     ga:slug for its file name"
                )
            })?;
        let kind = match view.optional(target, "documentType").await? {
            Some(Term::NamedNode(kind)) => DocumentType::from_node(&kind)?,
            Some(_) => bail!("{target} ga:documentType must be an IRI"),
            None => {
                bail!(
                    "PDF target {target} requires a ga:documentType for its \
                     file name"
                )
            },
        };
        let name = match Slug::declared(view, target).await? {
            Some(name) => name,
            None => {
                Self::family_slug(view, target).await?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "PDF target {target} requires a ga:slug, or a profile \
                         publishing a family with a ga:slug, for its file name"
                    )
                })?
            },
        };
        Ok(Self { authority, kind, name, edition })
    }

    async fn family_slug(
        view: &RevisionView,
        target: &NamedNode,
    ) -> Result<Option<Slug>> {
        let profile = view.node(target, "profile").await?;
        match view
            .optional_by(
                &profile,
                &NamedNode::new(format!("{PUBLICATION}publishesFamily"))?,
            )
            .await?
        {
            Some(Term::NamedNode(family)) => {
                Slug::declared(view, &family).await
            },
            Some(_) => bail!("artpub:publishesFamily must be an IRI"),
            None => Ok(None),
        }
    }

    /// The PDF file name, such as `ekgf-spec-dprod-pr23.pdf`.
    pub fn file_name(&self) -> String {
        let mut name = format!(
            "{}-{}-{}",
            self.authority,
            self.kind.slug(),
            self.name
        );
        if let Edition::Named(edition) = &self.edition {
            name.push('-');
            name.push_str(edition.as_str());
        }
        name.push_str(".pdf");
        name
    }

    /// Whether `value` has the shape of a generated PDF file name.
    pub fn is_file_name(value: &str) -> bool {
        value.strip_suffix(".pdf").is_some_and(|stem| {
            stem.contains('-') && Slug::try_new(stem).is_ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_join_declared_parts_and_reject_unsafe_slugs() -> Result<()> {
        let name = PdfName {
            authority: Slug::try_new("ekgf")?,
            kind:      DocumentType::Specification,
            name:      Slug::try_new("use-case-ontology")?,
            edition:   Edition::parse("pr-23")?,
        };
        assert_eq!(
            name.file_name(),
            "ekgf-spec-use-case-ontology-pr-23.pdf"
        );
        let default = PdfName {
            edition: Edition::parse("default")?,
            ..name.clone()
        };
        assert_eq!(
            default.file_name(),
            "ekgf-spec-use-case-ontology.pdf"
        );
        let branch = PdfName {
            edition: Edition::parse("feature-x")?,
            ..name.clone()
        };
        assert_ne!(branch.file_name(), name.file_name());
        assert!(PdfName::is_file_name(&name.file_name()));
        for invalid in ["", "EKGF", "a--b", "-a", "a_b", "a.b", "a/b"] {
            assert!(Slug::try_new(invalid).is_err(), "{invalid}");
        }
        Ok(())
    }
}
