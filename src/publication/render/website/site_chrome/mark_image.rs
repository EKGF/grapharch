//! The image of a brand mark: an embedded `data:` image published once beside
//! the website, or a link to an image the website or another host serves.
use {
    super::super::{
        site_base::SiteBases,
        site_link::{HEADER_SCHEMES, SiteLink},
    },
    anyhow::{Result, bail, ensure},
    base64::{Engine, engine::general_purpose::STANDARD},
    sha2::{Digest, Sha256},
};

/// The largest embedded brand image, before base64 encoding.
const MAX_EMBEDDED_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub(super) enum MarkImage {
    /// A validated image the website publishes at its route.
    Embedded { route: String, bytes: Vec<u8> },
    /// An image the website or another host already serves.
    Linked(SiteLink),
}

impl MarkImage {
    pub fn try_new(value: &str, bases: &SiteBases) -> Result<Self> {
        let Some(data) = value.strip_prefix("data:") else {
            return Ok(Self::Linked(SiteLink::authored(
                value,
                HEADER_SCHEMES,
                "brand image",
                bases,
            )?));
        };
        let Some((media, payload)) = data.split_once(',') else {
            bail!("brand image data URL lacks a payload")
        };
        let (mime, extension) = match media {
            "image/svg+xml;base64" => ("image/svg+xml", "svg"),
            "image/png;base64" => ("image/png", "png"),
            _ => {
                bail!(
                    "brand image data URL must be base64 image/svg+xml or \
                     image/png"
                )
            },
        };
        ensure!(
            payload.len() <= MAX_EMBEDDED_BYTES.div_ceil(3) * 4,
            "brand image exceeds {MAX_EMBEDDED_BYTES} bytes"
        );
        let bytes = STANDARD.decode(payload)?;
        ensure!(
            bytes.len() <= MAX_EMBEDDED_BYTES,
            "brand image exceeds {MAX_EMBEDDED_BYTES} bytes"
        );
        crate::publication::image_assets::validate(&bytes, mime)?;
        let digest = hex::encode(Sha256::digest(&bytes));
        Ok(Self::Embedded {
            route: format!("brand/mark-{}.{extension}", &digest[..16]),
            bytes,
        })
    }

    /// The `src` from a page whose path to the website root is `prefix`.
    pub fn src(&self, prefix: &str) -> String {
        match self {
            Self::Embedded { route, .. } => format!("{prefix}{route}"),
            Self::Linked(link) => link.href(prefix),
        }
    }

    /// The published route and content of an embedded image.
    pub fn embedded(&self) -> Option<(&str, &[u8])> {
        match self {
            Self::Embedded { route, bytes } => Some((route, bytes)),
            Self::Linked(_) => None,
        }
    }
}
