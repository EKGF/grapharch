//! A bounded publication index and byte restoration for retained image assets.
use {
    super::model::{Plan, node},
    crate::publication::{dataset::Dataset, term},
    anyhow::{Result, ensure},
    base64::{Engine, engine::general_purpose::STANDARD},
    oxrdf::{NamedNode, Term},
    sha2::{Digest, Sha256},
    std::{collections::HashMap, path::Path},
};

#[derive(Default)]
pub(super) struct Images {
    assets:     HashMap<NamedNode, Asset>,
    references: HashMap<(NamedNode, String), Reference>,
}
struct Asset {
    route: String,
    mime:  String,
}
struct Reference {
    url:   String,
    asset: Option<NamedNode>,
}

impl Images {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let mut images = Self::default();
        let mut routes = std::collections::HashSet::new();
        let mut assets = plan.assets().await?;
        while let Some(row) = assets.next().await? {
            ensure!(
                images.assets.len() < 256,
                "publication exceeds image asset budget"
            );
            let id = node(&row["asset"])?;
            let mime = plan.source.text(id, "format").await?;
            ensure!(
                matches!(
                    mime.as_str(),
                    "image/svg+xml" | "image/png" | "image/jpeg"
                ),
                "unsupported collected image MIME {mime}"
            );
            let route = plan.source.text(id, "route").await?;
            ensure!(
                route.len() <= 1024 &&
                    route.starts_with("assets/") &&
                    route.split('/').all(|part| {
                        !part.is_empty() &&
                            part.len() <= 255 &&
                            part != "." &&
                            part != ".." &&
                            part.chars().all(|c| {
                                c.is_ascii_alphanumeric() ||
                                    matches!(c, '.' | '-' | '_')
                            })
                    }),
                "invalid collected image asset route {route}"
            );
            let extension = match mime.as_str() {
                "image/svg+xml" => ".svg",
                "image/png" => ".png",
                _ => ".jpg",
            };
            ensure!(
                route.ends_with(extension) ||
                    (mime == "image/jpeg" && route.ends_with(".jpeg")),
                "image route extension disagrees with MIME"
            );
            ensure!(
                routes.insert(route.clone()),
                "conflicting collected image routes"
            );
            images.assets.insert(id.clone(), Asset { route, mime });
        }
        let entries = Dataset::logical_pattern(
            &plan.handle.snapshot,
            &format!(
                "{} {} ?entry . ?entry {} ?fragment",
                plan.handle.id,
                term("entry"),
                term("fragment")
            ),
            200,
        );
        let owners = plan.source.pattern(
            &format!("?fragment {}* ?owner", term("derivedFrom")),
            201,
        );
        let references = plan.source.pattern(&format!("?owner {} ?reference . ?reference <http://purl.org/dc/terms/identifier> ?original ; {} ?url", term("reference"), term("url")), 202);
        let asset = plan.source.pattern(
            &format!("?reference {} ?asset", term("asset")),
            203,
        );
        let mut rows = plan
            .dataset
            .select(format!(
                "SELECT DISTINCT ?fragment ?original ?url ?asset WHERE {{ \
                 {entries} {owners} {references} OPTIONAL {{ {asset} }} }}"
            ))
            .await?;
        let mut count = 0;
        let mut bytes = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= 16384,
                "publication reference index exceeds entry budget"
            );
            let fragment = node(&row["fragment"])?;
            let (Term::Literal(original), Term::Literal(url)) =
                (&row["original"], &row["url"])
            else {
                anyhow::bail!(
                    "collected reference identifier and URL must be literals"
                )
            };
            let original = original.value().to_owned();
            let url = url.value().to_owned();
            bytes += original.len() + url.len() + fragment.as_str().len();
            ensure!(
                bytes <= 8 * 1024 * 1024,
                "publication reference index exceeds byte budget"
            );
            let parsed = url::Url::parse(&url)?;
            let asset = row.get("asset").map(node).transpose()?.cloned();
            let retained = asset.as_ref().is_some_and(|asset| {
                parsed.scheme() == "urn" && url == asset.as_str()
            });
            ensure!(
                matches!(parsed.scheme(), "https" | "http" | "mailto") ||
                    retained,
                "unsafe collected reference URL {url}"
            );
            if let Some(asset) = &asset {
                ensure!(
                    matches!(parsed.scheme(), "https" | "http") || retained,
                    "unsafe collected image URL {url}"
                );
                ensure!(
                    images.assets.contains_key(asset),
                    "reference image is absent from the pinned publication \
                     assets"
                );
            }
            let key = (fragment.clone(), original);
            if let Some(previous) = images.references.get(&key) {
                ensure!(
                    previous.url == url && previous.asset == asset,
                    "ambiguous collected reference"
                );
            } else {
                images.references.insert(key, Reference { url, asset });
            }
        }
        Ok(images)
    }

    pub fn link(&self, fragment: &NamedNode, original: &str) -> Option<&str> {
        self.references
            .get(&(fragment.clone(), original.into()))
            .map(|reference| {
                match &reference.asset {
                    Some(asset) => {
                        self.assets
                            .get(asset)
                            .expect("indexed image asset")
                            .route
                            .as_str()
                    },
                    None => reference.url.as_str(),
                }
            })
    }

    pub fn image(&self, fragment: &NamedNode, original: &str) -> Result<&str> {
        let reference = self
            .references
            .get(&(fragment.clone(), original.into()))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Markdown image has no collected asset: {original}"
                )
            })?;
        let asset = reference.asset.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Markdown image reference has no retained image bytes: \
                 {original}"
            )
        })?;
        Ok(&self.assets.get(asset).expect("indexed image asset").route)
    }

    pub async fn write(
        &self,
        plan: &Plan,
        stage: &Path,
    ) -> Result<Vec<String>> {
        let mut files = Vec::new();
        let mut total = 0usize;
        let mut assets: Vec<_> = self.assets.iter().collect();
        assets.sort_by(|a, b| a.1.route.cmp(&b.1.route));
        for (id, asset) in assets {
            let size: usize =
                plan.source.text(id, "contentSize").await?.parse()?;
            ensure!(
                size > 0 &&
                    size <=
                        crate::publication::image_assets::MAX_IMAGE_BYTES,
                "collected image exceeds byte budget"
            );
            total += size;
            ensure!(
                total <= 32 * 1024 * 1024,
                "publication images exceed total byte budget"
            );
            let digest = plan.source.text(id, "contentDigest").await?;
            let pattern = plan.source.pattern(
                &format!(
                    "{id} {} ?chunk . ?chunk {} ?order ; {} ?data",
                    term("chunk"),
                    term("order"),
                    term("base64")
                ),
                204,
            );
            let mut rows = plan
                .dataset
                .select(format!(
                    "SELECT DISTINCT ?order ?data WHERE {{ {pattern} }} ORDER \
                     BY ?order"
                ))
                .await?;
            let mut bytes = Vec::with_capacity(size);
            let mut order = 0;
            while let Some(row) = rows.next().await? {
                ensure!(order < 128, "image chunk count exceeds budget");
                let (Term::Literal(position), Term::Literal(data)) =
                    (&row["order"], &row["data"])
                else {
                    anyhow::bail!("image chunks require literal order and data")
                };
                ensure!(
                    position.value().parse::<usize>()? == order,
                    "image chunks must have consecutive orders"
                );
                ensure!(
                    data.value().len() <= 64 * 1024,
                    "image chunk exceeds byte budget"
                );
                let chunk = STANDARD.decode(data.value())?;
                ensure!(
                    bytes.len() + chunk.len() <= size,
                    "image exceeds declared size"
                );
                bytes.extend_from_slice(&chunk);
                order += 1;
            }
            ensure!(
                bytes.len() == size &&
                    hex::encode(Sha256::digest(&bytes)) == digest,
                "image asset size or digest mismatch"
            );
            crate::publication::image_assets::validate(&bytes, &asset.mime)?;
            let path = stage.join(&asset.route);
            tokio::fs::create_dir_all(
                path.parent().expect("image asset parent"),
            )
            .await?;
            tokio::fs::write(path, bytes).await?;
            files.push(asset.route.clone());
        }
        Ok(files)
    }
}
