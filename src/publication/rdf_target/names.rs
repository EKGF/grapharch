//! Validated names of RDF publication: media types, file extensions, file
//! names and negotiated URL paths.
use {
    crate::publication::pdf_name::Slug,
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
    std::fmt,
};

/// An IANA media type such as `text/turtle`, without parameters.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct MediaType(String);

impl MediaType {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        let valid = value.len() <= 127 &&
            value.split_once('/').is_some_and(|(kind, subtype)| {
                !kind.is_empty() &&
                    kind.bytes().all(|byte| byte.is_ascii_lowercase()) &&
                    !subtype.is_empty() &&
                    subtype.bytes().all(|byte| {
                        byte.is_ascii_lowercase() ||
                            byte.is_ascii_digit() ||
                            matches!(byte, b'.' | b'+' | b'-')
                    })
            });
        ensure!(
            valid,
            "invalid media type {value:?}: use a lowercase type/subtype \
             without parameters"
        );
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl TryFrom<String> for MediaType {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> { Self::try_new(&value) }
}

impl From<MediaType> for String {
    fn from(value: MediaType) -> Self { value.0 }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A file extension without its leading dot, such as `ttl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileExtension(String);

impl FileExtension {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            (1..=16).contains(&value.len()) &&
                value.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit()
                }),
            "invalid file extension {value:?}: use 1 to 16 lowercase ASCII \
             letters and digits, without a dot"
        );
        Ok(Self(value.to_owned()))
    }
}

/// The name of one published RDF file: `<target slug>.<extension>`.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct RdfFileName(String);

impl RdfFileName {
    pub(crate) fn new(slug: &Slug, extension: &FileExtension) -> Self {
        Self(format!("{}.{}", slug.as_str(), extension.0))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl TryFrom<String> for RdfFileName {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        let (slug, extension) = value.rsplit_once('.').ok_or_else(|| {
            anyhow::anyhow!("RDF file {value:?} has no extension")
        })?;
        Ok(Self::new(
            &Slug::try_new(slug)?,
            &FileExtension::try_new(extension)?,
        ))
    }
}

impl From<RdfFileName> for String {
    fn from(value: RdfFileName) -> Self { value.0 }
}

impl fmt::Display for RdfFileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A URL path relative to the output root, such as `ontology/core` or
/// `ontology/core/`, at which a host negotiates the served representation.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct NegotiatedPath(String);

impl NegotiatedPath {
    pub(crate) fn try_new(value: &str) -> Result<Self> {
        let segments = value.strip_suffix('/').unwrap_or(value);
        ensure!(
            value.len() <= 512 &&
                !segments.is_empty() &&
                segments.split('/').all(|segment| {
                    !segment.is_empty() &&
                        segment != "." &&
                        segment != ".." &&
                        segment.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() ||
                                matches!(byte, b'.' | b'_' | b'-')
                        })
                }),
            "invalid ga:negotiatedPath {value:?}: use a relative URL path of \
             letters, digits, '.', '_' and '-' segments"
        );
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl TryFrom<String> for NegotiatedPath {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> { Self::try_new(&value) }
}

impl From<NegotiatedPath> for String {
    fn from(value: NegotiatedPath) -> Self { value.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_reject_malformed_values() {
        assert!(MediaType::try_new("text/turtle").is_ok());
        assert!(MediaType::try_new("application/ld+json").is_ok());
        for bad in ["text", "Text/turtle", "text/turtle; charset=utf-8", "/x"] {
            assert!(MediaType::try_new(bad).is_err(), "{bad}");
        }
        assert!(FileExtension::try_new("jsonld").is_ok());
        for bad in ["", ".ttl", "TTL", "t/l"] {
            assert!(FileExtension::try_new(bad).is_err(), "{bad}");
        }
        assert!(NegotiatedPath::try_new("ontology/core/").is_ok());
        for bad in ["", "/ontology", "a//b", "../a", "a/./b", "a b"] {
            assert!(NegotiatedPath::try_new(bad).is_err(), "{bad}");
        }
        assert!(RdfFileName::try_from("dprod-all.ttl".to_owned()).is_ok());
        assert!(RdfFileName::try_from("../x.ttl".to_owned()).is_err());
    }
}
