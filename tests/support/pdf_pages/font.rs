//! Font identities, normalized so that a document set in Times New Roman,
//! Arial and Courier New compares with one set in their metric twins.
use std::fmt;

/// A font family, without subset tag, weight or slant.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontFamily(String);

/// The typographic role a family plays, shared by metric-compatible families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontRole {
    Serif,
    Sans,
    Mono,
    /// A family that is neither one of the three roles nor a metric twin.
    Other,
}

impl FontFamily {
    /// The family of a PostScript font name such as
    /// `ABCDEF+TimesNewRomanPS-BoldMT`.
    pub fn from_postscript(name: &str) -> Self {
        let name = name.split_once('+').map_or(name, |(_, rest)| rest);
        let family = name.split(['-', ',']).next().unwrap_or(name);
        let family = ["PSMT", "PS", "MT"]
            .iter()
            .find_map(|suffix| family.strip_suffix(suffix))
            .unwrap_or(family);
        Self(family.to_owned())
    }

    /// The role of this family; metric twins share a role.
    pub fn role(&self) -> FontRole {
        match self.0.as_str() {
            "TimesNewRoman" | "Times" | "LiberationSerif" => FontRole::Serif,
            "Arial" | "Helvetica" | "LiberationSans" => FontRole::Sans,
            "CourierNew" | "Courier" | "LiberationMono" => FontRole::Mono,
            _ => FontRole::Other,
        }
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for FontFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[test]
fn subset_tags_and_style_suffixes_do_not_change_the_family() {
    for (name, family, role) in [
        (
            "ABCDEF+TimesNewRomanPS-BoldMT",
            "TimesNewRoman",
            FontRole::Serif,
        ),
        (
            "TimesNewRomanPSMT",
            "TimesNewRoman",
            FontRole::Serif,
        ),
        ("Arial-BoldMT", "Arial", FontRole::Sans),
        ("ArialMT", "Arial", FontRole::Sans),
        ("CourierNewPSMT", "CourierNew", FontRole::Mono),
        (
            "LiberationSerif-Bold",
            "LiberationSerif",
            FontRole::Serif,
        ),
        ("LiberationMono", "LiberationMono", FontRole::Mono),
        ("Cambria", "Cambria", FontRole::Other),
    ] {
        let parsed = FontFamily::from_postscript(name);
        assert_eq!(
            (parsed.as_str(), parsed.role()),
            (family, role),
            "{name}"
        );
    }
}
