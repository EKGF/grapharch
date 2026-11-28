//! A decorative inline icon recorded as an empty-alt builtin image URI.
use {
    super::{IconArtwork, IconBundle, IconPresentation, IconShortcode},
    anyhow::{Context, Result},
};

/// A bundled icon with its presentation, for example
/// `urn:grapharch:icon:material-cached:lg:middle`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Icon {
    pub(crate) shortcode:    IconShortcode,
    pub(crate) presentation: IconPresentation,
}
impl Icon {
    const PREFIX: &'static str = "urn:grapharch:icon:";

    /// A source shortcode, or `None` when the bundle has no such icon.
    pub(crate) fn from_source(
        shortcode: IconShortcode,
        presentation: IconPresentation,
    ) -> Result<Option<Self>> {
        Ok(IconBundle::builtin()
            .artwork(&shortcode)?
            .map(|_| Self { shortcode, presentation }))
    }

    pub(crate) fn parse_uri(uri: &str) -> Result<Option<Self>> {
        let Some(value) = uri.strip_prefix(Self::PREFIX) else {
            return Ok(None)
        };
        let mut tokens = value.split(':');
        let shortcode = IconShortcode::try_new(tokens.next().unwrap_or(""))?;
        let presentation = IconPresentation::from_tokens(tokens)?;
        Ok(Some(Self { shortcode, presentation }))
    }

    pub(crate) fn uri(&self) -> String {
        let mut uri = format!("{}{}", Self::PREFIX, self.shortcode);
        for token in self.presentation.tokens() {
            uri.push(':');
            uri.push_str(token);
        }
        uri
    }

    fn artwork(&self) -> Result<IconArtwork> {
        IconBundle::builtin()
            .artwork(&self.shortcode)?
            .with_context(|| {
                format!("unknown bundled icon: {}", self.shortcode)
            })
    }

    /// A standalone SVG document coloured with `currentColor`.
    pub(crate) fn svg(&self) -> Result<String> {
        let IconArtwork { view_box, paths } = self.artwork()?;
        Ok(format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view_box}\" \
             fill=\"currentColor\">{paths}</svg>"
        ))
    }

    /// Inline, accessibility-hidden HTML.
    pub(crate) fn html(&self) -> Result<String> {
        let IconArtwork { view_box, paths } = self.artwork()?;
        Ok(format!(
            "<svg class=\"{}\" xmlns=\"http://www.w3.org/2000/svg\" \
             viewBox=\"{view_box}\" fill=\"currentColor\" \
             aria-hidden=\"true\" focusable=\"false\" \
             data-icon=\"{}\">{paths}</svg>",
            self.presentation.css_classes(),
            self.shortcode
        ))
    }

    /// The PDF width, in em.
    pub(crate) const fn em(&self) -> f32 { self.presentation.size.em() }
}
