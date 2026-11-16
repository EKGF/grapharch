//! BCP 14 normative keywords (RFC 2119, RFC 8174). A target that declares
//! `ga:conformanceLanguage ga:Bcp14` presents standalone uppercase keywords
//! of its authored prose as `<em class="rfc2119">`, the markup native W3C
//! sources already carry, and styles both with the theme's keyword token.
mod conformance;
mod html;
mod keyword;
mod runs;
mod style;
mod typst;

#[cfg(test)]
mod tests;

pub(super) use conformance::Conformance;
