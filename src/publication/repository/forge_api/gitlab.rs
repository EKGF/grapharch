//! The GitLab REST API (v4): two requests per project.
use {
    super::{
        ForgeApi,
        ForgeMetadata,
        ForgeRelease,
        Visibility,
        client::{Client, below},
    },
    crate::publication::repository::{
        forge::ForgeRepository,
        package::{text, web_url, words},
    },
    anyhow::{Context, Result},
    async_trait::async_trait,
    chrono::{DateTime, Utc},
    serde::Deserialize,
};

pub(super) struct GitLab {
    base: url::Url,
}

impl GitLab {
    pub(super) fn new(base: url::Url) -> Self { Self { base } }
}

#[derive(Deserialize)]
struct License {
    key: Option<String>,
}

#[derive(Deserialize)]
struct Project {
    /// Required, so an error body is never read as metadata.
    #[serde(rename = "path_with_namespace")]
    _path:       String,
    description: Option<String>,
    #[serde(default)]
    topics:      Vec<String>,
    license:     Option<License>,
    visibility:  Option<String>,
    #[serde(default)]
    archived:    bool,
    star_count:  Option<u64>,
}

#[derive(Deserialize)]
struct Links {
    #[serde(rename = "self")]
    page: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    name:        Option<String>,
    tag_name:    String,
    released_at: Option<DateTime<Utc>>,
    #[serde(rename = "_links")]
    links:       Option<Links>,
}

/// GitLab reports lowercase license keys; SPDX spells them in mixed case.
fn spdx(key: &str) -> Option<String> {
    Some(
        match key {
            "mit" => "MIT",
            "apache-2.0" => "Apache-2.0",
            "bsd-2-clause" => "BSD-2-Clause",
            "bsd-3-clause" => "BSD-3-Clause",
            "gpl-2.0" => "GPL-2.0-only",
            "gpl-3.0" => "GPL-3.0-only",
            "lgpl-2.1" => "LGPL-2.1-only",
            "lgpl-3.0" => "LGPL-3.0-only",
            "agpl-3.0" => "AGPL-3.0-only",
            "mpl-2.0" => "MPL-2.0",
            "unlicense" => "Unlicense",
            "isc" => "ISC",
            _ => return None,
        }
        .to_owned(),
    )
}

#[async_trait]
impl ForgeApi for GitLab {
    fn base(&self) -> &url::Url { &self.base }

    async fn metadata(
        &self,
        client: &Client,
        repository: &ForgeRepository,
    ) -> Result<ForgeMetadata> {
        let id: String =
            url::form_urlencoded::byte_serialize(repository.path().as_bytes())
                .collect();
        let path = format!("projects/{id}");
        let body = client
            .get(
                below(&self.base, &format!("{path}?license=true"))?,
                &[],
            )
            .await?
            .with_context(|| {
                format!("GitLab project {} not found", repository.path())
            })?;
        let found: Project =
            serde_json::from_slice(&body).context("GitLab project response")?;
        let release = match client
            .get(
                below(
                    &self.base,
                    &format!(
                        "{path}/releases?per_page=1&order_by=released_at&\
                         sort=desc"
                    ),
                )?,
                &[],
            )
            .await?
        {
            Some(body) => {
                let releases: Vec<Release> = serde_json::from_slice(&body)
                    .context("GitLab releases response")?;
                releases.into_iter().next().map(|release| ForgeRelease {
                    name:  text(release.name.as_deref()),
                    notes: release
                        .links
                        .and_then(|links| links.page)
                        .and_then(|url| web_url(Some(&url)))
                        .or_else(|| {
                            crate::publication::repository::forge::GitRef::try_new(
                                &release.tag_name,
                            )
                            .ok()
                            .map(|tag| repository.release_url(&tag))
                        }),
                    tag:   release.tag_name,
                    date:  release.released_at,
                })
            },
            None => None,
        };
        Ok(ForgeMetadata {
            description:    text(found.description.as_deref()),
            homepage:       None,
            topics:         words(found.topics.iter().map(String::as_str)),
            license:        found
                .license
                .and_then(|l| l.key)
                .and_then(|k| spdx(&k)),
            visibility:     found
                .visibility
                .as_deref()
                .and_then(Visibility::parse),
            archived:       found.archived,
            stars:          found.star_count,
            discussions:    false,
            latest_release: release,
        })
    }
}
