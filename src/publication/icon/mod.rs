//! Bundled decorative icon sets; no source SVG or remote lookup.
mod bundle;
mod decorative;
mod licenses;
mod presentation;
mod shortcode;

pub(crate) use {
    bundle::{IconArtwork, IconBundle},
    decorative::Icon,
    licenses::write_licenses,
    presentation::IconPresentation,
    shortcode::IconShortcode,
};

#[cfg(test)]
mod tests;
