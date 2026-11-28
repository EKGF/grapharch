//! The forge API trait, bounded requests, and the access policy.
use {
    super::{ForgeCache, ForgeMetadata, github::GitHub, gitlab::GitLab},
    crate::publication::repository::forge::{ForgeKind, ForgeRepository},
    anyhow::{Context, Result, ensure},
    async_trait::async_trait,
    futures::StreamExt,
    std::time::Duration,
};

/// Largest accepted API response.
pub(super) const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// One forge's REST API.
#[async_trait]
pub(crate) trait ForgeApi: Send + Sync {
    /// Base URL every request resolves below.
    fn base(&self) -> &url::Url;

    /// Fetch repository metadata and the latest release.
    async fn metadata(
        &self,
        client: &Client,
        repository: &ForgeRepository,
    ) -> Result<ForgeMetadata>;
}

/// Whether a build may ask a forge API for metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ForgeAccess {
    /// Ask the forge, reusing a cached response for the same commit.
    Online,
    /// Use Git and manifests only; pinned and reproducible builds.
    Offline,
}

/// An HTTP client with an optional bearer token for one API.
pub(crate) struct Client {
    http:  reqwest::Client,
    token: Option<String>,
}

impl Client {
    pub(super) fn new(token: Option<String>) -> Result<Self> {
        Ok(Self {
            http:  reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::limited(3))
                .user_agent(concat!("GraphArch/", env!("CARGO_PKG_VERSION")))
                .build()?,
            token: token.filter(|token| !token.trim().is_empty()),
        })
    }

    /// GET `url`; `Ok(None)` for 404, an error for any other failure.
    pub(super) async fn get(
        &self,
        url: url::Url,
        headers: &[(&str, &str)],
    ) -> Result<Option<Vec<u8>>> {
        let mut request = self.http.get(url.clone());
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response =
            request.send().await.with_context(|| format!("GET {url}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = response
            .error_for_status()
            .with_context(|| format!("GET {url}"))?;
        ensure!(
            response.status().is_success(),
            "GET {url}: unexpected status {}",
            response.status()
        );
        let mut body = Vec::new(); // bounded: MAX_RESPONSE_BYTES.
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            ensure!(
                body.len() + chunk.len() <= MAX_RESPONSE_BYTES,
                "forge response {url} exceeds {MAX_RESPONSE_BYTES} bytes"
            );
            body.extend_from_slice(&chunk);
        }
        Ok(Some(body))
    }
}

/// Resolve `path` below `base`, refusing to escape it.
pub(super) fn below(base: &url::Url, path: &str) -> Result<url::Url> {
    let url = base.join(path)?;
    ensure!(
        url.as_str().starts_with(base.as_str()),
        "forge API path escapes its base"
    );
    Ok(url)
}

/// An API base URL: HTTPS, or HTTP on a loopback host for local mirrors.
pub(super) fn api_base(value: &str) -> Result<url::Url> {
    let mut base = url::Url::parse(value)
        .with_context(|| format!("forge API base {value}"))?;
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    let loopback = matches!(
        base.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]")
    );
    ensure!(
        (base.scheme() == "https" || (base.scheme() == "http" && loopback)) &&
            base.username().is_empty() &&
            base.password().is_none(),
        "forge API base must be HTTPS without credentials: {value}"
    );
    Ok(base)
}

/// The API and token the environment selects for `repository`'s forge:
/// `GITHUB_API_URL`/`GITHUB_TOKEN`, `CI_API_V4_URL`/`GITLAB_TOKEN`.
pub(crate) fn resolve(
    repository: &ForgeRepository,
    environment: &dyn Fn(&str) -> Option<String>,
) -> Result<(Box<dyn ForgeApi>, Client)> {
    match repository.kind() {
        ForgeKind::GitHub => {
            let base = environment("GITHUB_API_URL")
                .unwrap_or_else(|| "https://api.github.com/".into());
            Ok((
                Box::new(GitHub::new(api_base(&base)?)),
                Client::new(environment("GITHUB_TOKEN"))?,
            ))
        },
        ForgeKind::GitLab => {
            let base = environment("CI_API_V4_URL").unwrap_or_else(|| {
                format!("https://{}/api/v4/", repository.host())
            });
            Ok((
                Box::new(GitLab::new(api_base(&base)?)),
                Client::new(environment("GITLAB_TOKEN"))?,
            ))
        },
    }
}

/// Metadata for `repository` at `commit`: cached, else fetched and cached.
pub(crate) async fn fetch(
    repository: &ForgeRepository,
    commit: &str,
    cache: &ForgeCache,
    environment: &dyn Fn(&str) -> Option<String>,
) -> Result<ForgeMetadata> {
    let (api, client) = resolve(repository, environment)?;
    let key = cache.key(api.base(), repository, commit);
    if let Some(metadata) = cache.load(&key).await? {
        return Ok(metadata);
    }
    let metadata = api.metadata(&client, repository).await?;
    cache.store(&key, &metadata).await?;
    Ok(metadata)
}
