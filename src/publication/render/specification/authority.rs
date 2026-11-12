use {
    super::{
        super::{markup::html_escape, model::Plan},
        metadata::{optional, validate_url},
    },
    anyhow::{Result, ensure},
    base64::{Engine, engine::general_purpose::STANDARD},
    image::{ImageFormat, ImageReader, Limits},
    std::io::Cursor,
};

pub(super) struct Authority {
    pub color: String,
    pub name:  String,
    pub short: String,
    pub link:  String,
    pub logo:  String,
}

impl Authority {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let node = plan.source.node(&plan.target, "publisher").await?;
        let name =
            crate::publication::governance::name(&plan.source, &node).await?;
        let short =
            crate::publication::governance::acronym(&plan.source, &node)
                .await?;
        let color = optional(plan, &node, "accentColor")
            .await?
            .unwrap_or_else(|| "#005a9c".into());
        ensure!(
            color.len() == 7 &&
                color.starts_with('#') &&
                color[1..].bytes().all(|b| b.is_ascii_hexdigit()),
            "accentColor must be #RRGGBB"
        );
        let url = optional(plan, &node, "url").await?;
        let link = if let Some(url) = url {
            validate_url(&url)?;
            format!(
                "<a href=\"{}\">{}</a>",
                html_escape(&url),
                html_escape(&short)
            )
        } else {
            html_escape(&short)
        };
        let logo = match Logo::read(plan, &node).await? {
            Some(logo) => {
                format!(
                    "<img class=\"authority-logo\" src=\"{}\" alt=\"{}\">",
                    html_escape(&logo.data_uri),
                    html_escape(&name)
                )
            },
            None => String::new(),
        };
        Ok(Self { color, name, short, link, logo })
    }
}

/// A validated authority `ga:logo`: a PNG or SVG data URI of at most 256 KiB.
pub(in crate::publication::render) struct Logo {
    pub data_uri:  String,
    pub bytes:     Vec<u8>,
    pub extension: &'static str,
}

impl Logo {
    pub(in crate::publication::render) async fn read(
        plan: &Plan,
        authority: &oxrdf::NamedNode,
    ) -> Result<Option<Self>> {
        let Some(logo) = plan.source.optional(authority, "logo").await? else {
            return Ok(None);
        };
        let oxrdf::Term::Literal(logo) = logo else {
            anyhow::bail!(
                "authority logo must be a PNG or SVG data URI literal"
            )
        };
        let data_uri = logo.value().to_owned();
        ensure!(
            data_uri.len() <= 350_000,
            "authority logo exceeds 256 KiB encoded-image budget"
        );
        let (mime, extension, encoded) = if let Some(encoded) =
            data_uri.strip_prefix("data:image/png;base64,")
        {
            ("image/png", "png", encoded)
        } else if let Some(encoded) =
            data_uri.strip_prefix("data:image/svg+xml;base64,")
        {
            ("image/svg+xml", "svg", encoded)
        } else {
            anyhow::bail!("authority logo must be a PNG or SVG data URI")
        };
        let bytes = STANDARD.decode(encoded)?;
        ensure!(
            bytes.len() <= 256 * 1024,
            "authority logo exceeds 256 KiB"
        );
        let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            if mime == "image/png" {
                let mut reader = ImageReader::with_format(
                    Cursor::new(&bytes),
                    ImageFormat::Png,
                );
                let mut limits = Limits::default();
                limits.max_image_width = Some(2048);
                limits.max_image_height = Some(2048);
                limits.max_alloc = Some(32 * 1024 * 1024);
                reader.limits(limits);
                reader.decode()?;
            } else {
                crate::publication::image_assets::validate(&bytes, mime)?;
            }
            Ok(bytes)
        })
        .await??;
        Ok(Some(Self { data_uri, bytes, extension }))
    }
}
