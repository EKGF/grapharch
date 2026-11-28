//! Material attribute-list classes that size and align an inline icon.

/// An icon size, following Material for MkDocs' `--md-icon-size` steps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum IconSize {
    /// The running-text size.
    #[default]
    Normal,
    /// `.lg`
    Large,
    /// `.xl`
    ExtraLarge,
    /// `.xxl`
    ExtraExtraLarge,
}
impl IconSize {
    /// The rendered height and width, in em.
    pub(crate) const fn em(self) -> f32 {
        match self {
            Self::Normal => 1.125,
            Self::Large => 1.5,
            Self::ExtraLarge => 2.25,
            Self::ExtraExtraLarge => 3.0,
        }
    }

    const fn token(self) -> Option<&'static str> {
        match self {
            Self::Normal => None,
            Self::Large => Some("lg"),
            Self::ExtraLarge => Some("xl"),
            Self::ExtraExtraLarge => Some("xxl"),
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "lg" => Some(Self::Large),
            "xl" => Some(Self::ExtraLarge),
            "xxl" => Some(Self::ExtraExtraLarge),
            _ => None,
        }
    }
}

/// How an inline icon is sized and aligned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct IconPresentation {
    pub(crate) size:   IconSize,
    pub(crate) middle: bool,
}
impl IconPresentation {
    /// Apply one attribute-list class. Returns `false` for a class GraphArch
    /// does not reproduce.
    pub(crate) fn apply_class(&mut self, class: &str) -> bool {
        if class == "middle" {
            self.middle = true;
            return true;
        }
        match IconSize::from_token(class) {
            Some(size) => {
                self.size = size;
                true
            },
            None => false,
        }
    }

    /// The URI suffix tokens, in a canonical order.
    pub(crate) fn tokens(self) -> impl Iterator<Item = &'static str> {
        self.size
            .token()
            .into_iter()
            .chain(self.middle.then_some("middle"))
    }

    /// Parse URI suffix tokens written by [`Self::tokens`].
    pub(crate) fn from_tokens<'a>(
        tokens: impl Iterator<Item = &'a str>,
    ) -> anyhow::Result<Self> {
        let mut presentation = Self::default();
        for token in tokens {
            anyhow::ensure!(
                presentation.apply_class(token),
                "unknown icon presentation token: {token}"
            );
        }
        Ok(presentation)
    }

    /// The HTML classes of the rendered SVG.
    pub(crate) fn css_classes(self) -> String {
        let mut classes = String::from("inline-icon");
        for token in self.tokens() {
            classes.push_str(" inline-icon-");
            classes.push_str(token);
        }
        classes
    }
}
