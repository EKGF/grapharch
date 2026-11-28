//! A validated icon shortcode such as `material-cached`.
use {
    anyhow::{Result, ensure},
    std::fmt,
};

/// An icon set Material for MkDocs exposes through shortcodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconSet {
    /// Material Design Icons (`material-`).
    Material,
    /// GitHub Octicons (`octicons-`).
    Octicons,
    /// Simple Icons brand logos (`simple-`).
    Simple,
    /// Font Awesome Free brands (`fontawesome-brands-`).
    FontAwesomeBrands,
    /// Font Awesome Free regular (`fontawesome-regular-`).
    FontAwesomeRegular,
    /// Font Awesome Free solid (`fontawesome-solid-`).
    FontAwesomeSolid,
}
impl IconSet {
    const ALL: [Self; 6] = [
        Self::Material,
        Self::Octicons,
        Self::Simple,
        Self::FontAwesomeBrands,
        Self::FontAwesomeRegular,
        Self::FontAwesomeSolid,
    ];

    /// The shortcode prefix, including its trailing hyphen.
    pub(crate) const fn prefix(self) -> &'static str {
        match self {
            Self::Material => "material-",
            Self::Octicons => "octicons-",
            Self::Simple => "simple-",
            Self::FontAwesomeBrands => "fontawesome-brands-",
            Self::FontAwesomeRegular => "fontawesome-regular-",
            Self::FontAwesomeSolid => "fontawesome-solid-",
        }
    }

    fn of(shortcode: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|set| shortcode.starts_with(set.prefix()))
    }
}

/// A syntactically valid shortcode of a bundled icon set. Whether the bundle
/// holds artwork for it is a separate lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IconShortcode {
    value: String,
    set:   IconSet,
}
impl IconShortcode {
    const MAX_LEN: usize = 128;

    pub(crate) fn try_new(value: &str) -> Result<Self> {
        ensure!(
            value.len() <= Self::MAX_LEN,
            "icon shortcode exceeds {} bytes",
            Self::MAX_LEN
        );
        ensure!(
            value.bytes().all(|byte| {
                byte.is_ascii_lowercase() ||
                    byte.is_ascii_digit() ||
                    matches!(byte, b'-' | b'_')
            }),
            "icon shortcode contains unsupported characters: {value}"
        );
        let set = IconSet::of(value).ok_or_else(|| {
            anyhow::anyhow!("icon shortcode has no bundled set: {value}")
        })?;
        ensure!(
            value.len() > set.prefix().len(),
            "icon shortcode names no icon: {value}"
        );
        Ok(Self { value: value.into(), set })
    }

    pub(crate) fn as_str(&self) -> &str { &self.value }

    pub(crate) const fn set(&self) -> IconSet { self.set }

    /// The bundle shard holding this icon: set prefix plus the first name
    /// character, matching `generate.py`.
    pub(crate) fn shard(&self) -> &str {
        &self.value[..self.set.prefix().len() + 1]
    }
}
impl fmt::Display for IconShortcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}
