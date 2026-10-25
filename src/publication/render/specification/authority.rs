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
        let logo = if let Some(logo) =
            plan.source.optional(&node, "logo").await?
        {
            let oxrdf::Term::Literal(logo) = logo else {
                anyhow::bail!("authority logo must be a PNG data URI literal")
            };
            let logo = logo.value().to_owned();
            ensure!(
                logo.len() <= 350_000,
                "authority logo exceeds 256 KiB encoded-image budget"
            );
            let encoded =
                logo.strip_prefix("data:image/png;base64,").ok_or_else(
                    || anyhow::anyhow!("authority logo must be a PNG data URI"),
                )?;
            let bytes = STANDARD.decode(encoded)?;
            ensure!(
                bytes.len() <= 256 * 1024,
                "authority logo exceeds 256 KiB"
            );
            tokio::task::spawn_blocking(move || -> Result<()> {
                let mut reader = ImageReader::with_format(
                    Cursor::new(bytes),
                    ImageFormat::Png,
                );
                let mut limits = Limits::default();
                limits.max_image_width = Some(2048);
                limits.max_image_height = Some(2048);
                limits.max_alloc = Some(32 * 1024 * 1024);
                reader.limits(limits);
                reader.decode()?;
                Ok(())
            })
            .await??;
            format!(
                "<img class=\"authority-logo\" src=\"{}\" alt=\"{}\">",
                html_escape(&logo),
                html_escape(&name)
            )
        } else {
            String::new()
        };
        Ok(Self { color, name, short, link, logo })
    }
}
