//! One bounded HTTPS image, retained in the graph rather than fetched by
//! renderers.
use {
    super::{Collector, SourceIdentity},
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        image_assets::{self, MAX_IMAGE_BYTES},
        term,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    base64::{Engine, engine::general_purpose::STANDARD},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::time::Duration,
};
pub struct HttpImage {
    project:   String,
    url:       String,
    processor: String,
    id:        NamedNode,
}
impl HttpImage {
    pub fn new(project: &str, url: &str) -> Result<Self> {
        let parsed = url::Url::parse(url)?;
        ensure!(
            parsed.scheme() == "https" &&
                parsed.username().is_empty() &&
                parsed.password().is_none() &&
                parsed.fragment().is_none(),
            "images require credential-free HTTPS URLs"
        );
        Ok(Self {
            project:   project.into(),
            url:       url.into(),
            processor: identity("http-image-collector", &[project, url])
                .into_string(),
            id:        identity("image-asset", &[project, url]),
        })
    }

    pub fn id(&self) -> NamedNode { self.id.clone() }
}
#[async_trait]
impl Collector for HttpImage {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: &self.processor,
            source:    &self.url,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 3 {
                    attempt.error("image redirect limit exceeded")
                } else if attempt.url().scheme() != "https" ||
                    !attempt.url().username().is_empty() ||
                    attempt.url().password().is_some()
                {
                    attempt.error("unsafe image redirect")
                } else {
                    attempt.follow()
                }
            }))
            .build()?;
        let mut response = client
            .get(&self.url)
            .header(
                reqwest::header::ACCEPT,
                "image/svg+xml,image/png,image/jpeg",
            )
            .send()
            .await?
            .error_for_status()?;
        ensure!(
            response
                .content_length()
                .is_none_or(|n| n <= MAX_IMAGE_BYTES as u64),
            "image exceeds 2 MiB budget"
        );
        let mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .ok_or_else(|| {
                anyhow::anyhow!("image response has no Content-Type")
            })?
            .to_str()?
            .split(';')
            .next()
            .unwrap()
            .trim()
            .to_owned();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= MAX_IMAGE_BYTES,
                "image exceeds 2 MiB budget"
            );
            bytes.extend_from_slice(&chunk);
        }
        ImageAsset {
            project:   &self.project,
            processor: &self.processor,
            source:    &self.url,
            id:        &self.id,
        }
        .write(&bytes, &mime, stream)
        .await
    }
}

pub(crate) struct ImageAsset<'a> {
    pub project:   &'a str,
    pub processor: &'a str,
    pub source:    &'a str,
    pub id:        &'a NamedNode,
}
impl ImageAsset<'_> {
    fn quad(
        &self,
        subject: &NamedNode,
        field: &str,
        value: impl Into<Term>,
    ) -> Quad {
        Quad::new(
            subject.clone(),
            if field == "type" {
                NamedNode::new_unchecked(RDF_TYPE)
            } else {
                term(field)
            },
            value,
            GraphName::DefaultGraph,
        )
    }

    pub async fn write(
        &self,
        bytes: &[u8],
        mime: &str,
        stream: &mut ContributionStream,
    ) -> Result<String> {
        image_assets::validate(bytes, mime)?;
        let digest = hex::encode(Sha256::digest(bytes));
        let extension = match mime {
            "image/svg+xml" => "svg",
            "image/png" => "png",
            "image/jpeg" => "jpg",
            _ => unreachable!(),
        };
        let route = format!(
            "assets/image-{}.{}",
            hex::encode(Sha256::digest(self.source.as_bytes())),
            extension
        );
        let contribution = identity("contribution", &[
            self.project,
            self.processor,
            self.source,
        ]);
        for quad in [
            self.quad(self.id, "type", term("BinaryAsset")),
            self.quad(
                self.id,
                "route",
                Literal::new_simple_literal(route),
            ),
            self.quad(
                self.id,
                "format",
                Literal::new_simple_literal(mime),
            ),
            self.quad(
                self.id,
                "sourcePath",
                Literal::new_simple_literal(self.source),
            ),
            self.quad(
                self.id,
                "contentDigest",
                Literal::new_simple_literal(&digest),
            ),
            self.quad(
                self.id,
                "contentSize",
                Literal::from(bytes.len() as i64),
            ),
            self.quad(self.id, "contribution", contribution),
        ] {
            stream.write(quad).await?;
        }
        for (index, bytes) in bytes.chunks(48 * 1024).enumerate() {
            let chunk = identity("image-chunk", &[
                self.id.as_str(),
                &index.to_string(),
            ]);
            for quad in [
                self.quad(self.id, "chunk", chunk.clone()),
                self.quad(&chunk, "order", Literal::from(index as u32)),
                self.quad(
                    &chunk,
                    "base64",
                    Literal::new_simple_literal(STANDARD.encode(bytes)),
                ),
            ] {
                stream.write(quad).await?;
            }
        }
        Ok(format!("retained-image-v1:{digest}"))
    }
}
