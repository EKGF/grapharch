//! The GitHub REST API: two requests per repository.
use {
    super::{
        ForgeApi,
        ForgeMetadata,
        ForgeRelease,
        Visibility,
        client::{Client, below},
    },
    crate::publication::repository::{forge::ForgeRepository, package::text},
    anyhow::{Context, Result},
    async_trait::async_trait,
    chrono::{DateTime, Utc},
    serde::Deserialize,
};

const HEADERS: &[(&str, &str)] = &[
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", "2022-11-28"),
];

pub(super) struct GitHub {
    base: url::Url,
}

impl GitHub {
    pub(super) fn new(base: url::Url) -> Self { Self { base } }
}

#[derive(Deserialize)]
struct License {
    spdx_id: Option<String>,
}

#[derive(Deserialize)]
struct Repository {
    /// Required, so an error or redirect body is never read as metadata.
    #[serde(rename = "full_name")]
    _full_name:       String,
    description:      Option<String>,
    homepage:         Option<String>,
    #[serde(default)]
    topics:           Vec<String>,
    license:          Option<License>,
    visibility:       Option<String>,
    #[serde(default)]
    archived:         bool,
    stargazers_count: Option<u64>,
    #[serde(default)]
    has_discussions:  bool,
}

#[derive(Deserialize)]
struct Release {
    name:         Option<String>,
    tag_name:     String,
    published_at: Option<DateTime<Utc>>,
    html_url:     Option<String>,
}

#[async_trait]
impl ForgeApi for GitHub {
    fn base(&self) -> &url::Url { &self.base }

    async fn metadata(
        &self,
        client: &Client,
        repository: &ForgeRepository,
    ) -> Result<ForgeMetadata> {
        let path = format!(
            "repos/{}/{}",
            repository.namespace(),
            repository.name()
        );
        let body = client
            .get(below(&self.base, &path)?, HEADERS)
            .await?
            .with_context(|| {
                format!(
                    "GitHub repository {} not found",
                    repository.path()
                )
            })?;
        let found: Repository = serde_json::from_slice(&body)
            .context("GitHub repository response")?;
        let release = match client
            .get(
                below(&self.base, &format!("{path}/releases/latest"))?,
                HEADERS,
            )
            .await?
        {
            Some(body) => {
                let release: Release = serde_json::from_slice(&body)
                    .context("GitHub release response")?;
                Some(ForgeRelease {
                    name:  text(release.name.as_deref()),
                    tag:   release.tag_name,
                    date:  release.published_at,
                    notes: release
                        .html_url
                        .and_then(|url| url::Url::parse(&url).ok()),
                })
            },
            None => None,
        };
        Ok(ForgeMetadata {
            description:    text(found.description.as_deref()),
            homepage:       crate::publication::repository::package::web_url(
                found.homepage.as_deref(),
            ),
            topics:         crate::publication::repository::package::words(
                found.topics.iter().map(String::as_str),
            ),
            license:        found.license.and_then(|license| license.spdx_id),
            visibility:     found
                .visibility
                .as_deref()
                .and_then(Visibility::parse),
            archived:       found.archived,
            stars:          found.stargazers_count,
            discussions:    found.has_discussions,
            latest_release: release,
        })
    }
}
