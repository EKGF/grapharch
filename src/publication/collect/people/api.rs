//! A configured person-API base URL and bounded JSON requests against it.
use {
    anyhow::{Context, Result, ensure},
    futures::StreamExt,
    std::time::Duration,
};

/// The base URL a project explicitly configured as a person collector's
/// `ga:source`. Plain HTTP is accepted only for a loopback host, so tests and
/// local mirrors can stand in for the public service.
#[derive(Clone)]
pub(super) struct Endpoint {
    base: url::Url,
}

impl Endpoint {
    pub(super) fn try_new(source: &str) -> Result<Self> {
        let base = url::Url::parse(source)
            .with_context(|| format!("person API source {source}"))?;
        let loopback = matches!(
            base.host(),
            Some(url::Host::Ipv4(address)) if address.is_loopback()
        ) || matches!(
            base.host(),
            Some(url::Host::Ipv6(address)) if address.is_loopback()
        ) || base.host_str() == Some("localhost");
        ensure!(
            (base.scheme() == "https" || (base.scheme() == "http" && loopback)) &&
                base.host_str().is_some() &&
                base.username().is_empty() &&
                base.password().is_none() &&
                base.query().is_none() &&
                base.fragment().is_none() &&
                base.path().ends_with('/'),
            "person API source must be an HTTPS base URL ending in '/', \
             without credentials, query, or fragment"
        );
        Ok(Self { base })
    }

    /// Resolve an API path below the configured base.
    pub(super) fn url(&self, path: &str) -> Result<url::Url> {
        let url = self.base.join(path)?;
        ensure!(
            url.as_str().starts_with(self.base.as_str()),
            "person API path escapes its configured base"
        );
        Ok(url)
    }

    pub(super) fn host(&self) -> &str {
        self.base
            .host_str()
            .expect("validated endpoints have a host")
    }

    pub(super) fn client() -> Result<reqwest::Client> {
        Ok(reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("GraphArch/", env!("CARGO_PKG_VERSION")))
            .build()?)
    }
}

/// Read one successful JSON response, rejecting bodies above `limit` bytes.
pub(super) async fn get(
    request: reqwest::RequestBuilder,
    limit: usize,
) -> Result<Vec<u8>> {
    let response = request.send().await?;
    let url = response.url().to_string();
    let response = response
        .error_for_status()
        .with_context(|| format!("person API request {url}"))?;
    ensure!(
        response
            .content_length()
            .is_none_or(|length| length <= limit as u64),
        "person API response {url} exceeds {limit} bytes"
    );
    let mut bytes = Vec::new(); // bounded: rejected above `limit` bytes.
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        ensure!(
            bytes.len() + chunk.len() <= limit,
            "person API response {url} exceeds {limit} bytes"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
