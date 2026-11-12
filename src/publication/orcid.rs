//! ORCID iD identities shared by collection and credit rendering.
//!
//! An ORCID iD is sixteen characters in four hyphenated groups. The last
//! character is an ISO 7064 MOD 11-2 check character. ORCID publishes
//! `https://orcid.org/<id>` as the canonical IRI; older sources often use the
//! `http` scheme for the same identity.
use {
    anyhow::{Result, bail, ensure},
    oxrdf::NamedNode,
    std::fmt,
};

/// A checksum-validated ORCID iD, independent of the IRI scheme it was read
/// from.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrcidId(String);

impl OrcidId {
    /// Host of every ORCID iD IRI.
    pub const HOST: &'static str = "orcid.org";

    /// Whether an IRI claims to be an ORCID iD, before validating it.
    pub fn is_orcid_iri(iri: &str) -> bool {
        url::Url::parse(iri).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https") &&
                url.host_str() == Some(Self::HOST)
        })
    }

    /// Validate a bare `XXXX-XXXX-XXXX-XXXX` identifier and its check
    /// character.
    pub fn try_new(value: &str) -> Result<Self> {
        let bytes = value.as_bytes();
        ensure!(
            bytes.len() == 19 &&
                [4, 9, 14].iter().all(|&index| bytes[index] == b'-'),
            "ORCID iD {value} must have the form XXXX-XXXX-XXXX-XXXX"
        );
        let characters: Vec<u8> =
            bytes.iter().copied().filter(|&byte| byte != b'-').collect(); // bounded: sixteen identifier characters.
        ensure!(
            characters.len() == 16 &&
                characters[..15].iter().all(u8::is_ascii_digit) &&
                (characters[15].is_ascii_digit() || characters[15] == b'X'),
            "ORCID iD {value} must contain fifteen digits and a check \
             character"
        );
        let expected = Self::check_character(&characters[..15]);
        ensure!(
            characters[15] == expected,
            "ORCID iD {value} fails its ISO 7064 MOD 11-2 checksum"
        );
        Ok(Self(value.to_owned()))
    }

    /// Validate an `http(s)://orcid.org/<id>` IRI.
    pub fn from_iri(iri: &str) -> Result<Self> {
        let url = url::Url::parse(iri)?;
        ensure!(
            matches!(url.scheme(), "http" | "https") &&
                url.host_str() == Some(Self::HOST) &&
                url.port().is_none() &&
                url.username().is_empty() &&
                url.password().is_none() &&
                url.query().is_none() &&
                url.fragment().is_none(),
            "ORCID iD IRI {iri} must be http(s)://orcid.org/<id>"
        );
        let Some(id) = url.path().strip_prefix('/') else {
            bail!("ORCID iD IRI {iri} has no identifier")
        };
        Self::try_new(id)
    }

    /// The identifier without scheme or host.
    pub fn as_str(&self) -> &str { &self.0 }

    /// The canonical `https://orcid.org/<id>` IRI that identifies the
    /// person.
    pub fn canonical(&self) -> NamedNode {
        NamedNode::new_unchecked(format!("https://{}/{}", Self::HOST, self.0))
    }

    fn check_character(digits: &[u8]) -> u8 {
        let total = digits.iter().fold(0u32, |total, digit| {
            (total + u32::from(digit - b'0')) * 2
        });
        match (12 - total % 11) % 11 {
            10 => b'X',
            value => b'0' + value as u8,
        }
    }
}

impl fmt::Display for OrcidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::OrcidId;

    #[test]
    fn checksums_follow_iso_7064_mod_11_2() {
        for valid in [
            "0000-0003-2309-5068",
            "0000-0002-1825-0097",
            "0000-0001-5109-3700",
            "0000-0002-1694-233X",
        ] {
            assert_eq!(OrcidId::try_new(valid).unwrap().as_str(), valid);
        }
        for invalid in [
            "0000-0003-2309-5069",
            "0000-0002-1694-2330",
            "0000000323095068",
            "0000-0003-2309-506",
            "0000-0003-2309-50x8",
            "0000-0003-2309-506x",
        ] {
            assert!(OrcidId::try_new(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn http_and_https_iris_share_one_canonical_identity() {
        let http =
            OrcidId::from_iri("http://orcid.org/0000-0003-2309-5068").unwrap();
        let https =
            OrcidId::from_iri("https://orcid.org/0000-0003-2309-5068").unwrap();
        assert_eq!(http, https);
        assert_eq!(
            http.canonical().as_str(),
            "https://orcid.org/0000-0003-2309-5068"
        );
        for rejected in [
            "https://orcid.org/0000-0003-2309-5068/",
            "https://orcid.org/0000-0003-2309-5068?lang=en",
            "https://www.orcid.org/0000-0003-2309-5068",
            "ftp://orcid.org/0000-0003-2309-5068",
        ] {
            assert!(OrcidId::from_iri(rejected).is_err(), "{rejected}");
        }
        assert!(OrcidId::is_orcid_iri("http://orcid.org/anything"));
        assert!(!OrcidId::is_orcid_iri(
            "https://github.com/someone"
        ));
    }
}
