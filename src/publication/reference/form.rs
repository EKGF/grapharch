//! How a reference was written, before anything is resolved.
use url::Url;

/// The original form of a reference, as authored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceForm {
    /// `#heading`
    FragmentOnly(String),
    /// `guide.md#x`, `../src/main.rs`: relative to the referring source.
    Relative { path: String, fragment: Option<String> },
    /// `/docs/guide/`: relative to the site or repository root.
    RootRelative { path: String, fragment: Option<String> },
    /// `https://example.org/x`
    Absolute(Url),
    /// `//example.org/x`, resolved as HTTPS.
    SchemeRelative(Url),
    /// `mailto:` and `tel:` links.
    Contact(Url),
    /// Any other scheme, such as `urn:`, `ftp:` or `javascript:`.
    OtherScheme(String),
}

impl ReferenceForm {
    pub(crate) fn parse(value: &str) -> Self {
        let value = value.trim();
        if let Some(fragment) = value.strip_prefix('#') {
            return Self::FragmentOnly(fragment.to_owned());
        }
        if let Some(rest) = value.strip_prefix("//") &&
            let Ok(url) = Url::parse(&format!("https://{rest}"))
        {
            return Self::SchemeRelative(url);
        }
        let split = |value: &str| {
            match value.split_once('#') {
                Some((path, fragment)) => {
                    (path.to_owned(), Some(fragment.to_owned()))
                },
                None => (value.to_owned(), None),
            }
        };
        if let Some(rest) = value.strip_prefix('/') {
            let (path, fragment) = split(rest);
            return Self::RootRelative { path, fragment };
        }
        if let Ok(url) = Url::parse(value) {
            return match url.scheme() {
                "http" | "https" => Self::Absolute(url),
                "mailto" | "tel" => Self::Contact(url),
                scheme => Self::OtherScheme(scheme.to_owned()),
            };
        }
        let (path, fragment) = split(value);
        Self::Relative { path, fragment }
    }

    /// The form's local name in the GraphArch vocabulary.
    pub(crate) fn term(&self) -> &'static str {
        match self {
            Self::FragmentOnly(_) => "FragmentForm",
            Self::Relative { .. } => "RelativeForm",
            Self::RootRelative { .. } => "RootRelativeForm",
            Self::Absolute(_) => "AbsoluteForm",
            Self::SchemeRelative(_) => "SchemeRelativeForm",
            Self::Contact(_) => "ContactForm",
            Self::OtherScheme(_) => "OtherSchemeForm",
        }
    }
}
