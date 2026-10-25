//! Shared HTTP policy for explicitly configured SPARQL sources and stores.
use {
    anyhow::{Context, Result, ensure},
    reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue},
    serde::Deserialize,
    std::{
        io::{self, Read},
        time::Duration,
    },
};

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Authentication {
    Bearer { token_env: String },
    Basic { username_env: String, password_env: String },
}
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpPolicy {
    pub authentication:     Option<Authentication>,
    pub timeout_seconds:    u64,
    pub max_response_bytes: u64,
}
impl Default for HttpPolicy {
    fn default() -> Self {
        Self {
            authentication:     None,
            timeout_seconds:    30,
            max_response_bytes: 128 * 1024 * 1024,
        }
    }
}
impl HttpPolicy {
    pub(crate) fn headers(&self) -> Result<HeaderMap> {
        ensure!(
            (1..=3600).contains(&self.timeout_seconds),
            "SPARQL timeout must be between 1 and 3600 seconds"
        );
        ensure!(
            self.max_response_bytes > 0,
            "SPARQL response byte limit must be positive"
        );
        let mut headers = HeaderMap::new();
        if let Some(auth) = &self.authentication {
            let value = match auth {
                Authentication::Bearer { token_env } => {
                    format!("Bearer {}", secret(token_env)?)
                },
                Authentication::Basic { username_env, password_env } => {
                    use base64::Engine;
                    let user = secret(username_env)?;
                    ensure!(
                        !user.contains(':'),
                        "HTTP Basic username must not contain a colon"
                    );
                    format!(
                        "Basic {}",
                        base64::engine::general_purpose::STANDARD.encode(
                            format!("{user}:{}", secret(password_env)?)
                        )
                    )
                },
            };
            let mut value = HeaderValue::from_str(&value)
                .context("invalid SPARQL authorization value")?;
            value.set_sensitive(true);
            headers.insert(AUTHORIZATION, value);
        }
        Ok(headers)
    }

    pub(crate) fn asynchronous(&self) -> Result<reqwest::Client> {
        Ok(reqwest::Client::builder()
            .default_headers(self.headers()?)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(self.timeout_seconds))
            .build()?)
    }

    pub(crate) fn blocking(&self) -> Result<reqwest::blocking::Client> {
        Ok(reqwest::blocking::Client::builder()
            .default_headers(self.headers()?)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(self.timeout_seconds))
            .build()?)
    }
}
fn secret(name: &str) -> Result<String> {
    ensure!(
        !name.is_empty(),
        "credential environment variable name must not be empty"
    );
    let value = std::env::var(name).with_context(|| {
        format!(
            "required credential environment variable {name} is unavailable"
        )
    })?;
    ensure!(
        !value.is_empty(),
        "credential environment variable {name} is empty"
    );
    Ok(value)
}
pub(crate) fn endpoint(value: &str) -> Result<url::Url> {
    let url = url::Url::parse(value)?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "SPARQL endpoint must use HTTP or HTTPS"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "SPARQL credentials belong in the authentication configuration, not \
         the URL"
    );
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "configure a plain SPARQL endpoint URL without query parameters or \
         fragments"
    );
    Ok(url)
}
pub(crate) struct LimitedRead<R> {
    inner:     R,
    remaining: u64,
}
impl<R> LimitedRead<R> {
    pub fn new(inner: R, limit: u64) -> Self {
        Self { inner, remaining: limit }
    }
}
impl<R: Read> Read for LimitedRead<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let limit = buffer.len().min(self.remaining.saturating_add(1) as usize);
        let n = self.inner.read(&mut buffer[..limit])?;
        if n as u64 > self.remaining {
            return Err(io::Error::other(
                "SPARQL response exceeds configured byte limit",
            ));
        }
        self.remaining -= n as u64;
        Ok(n)
    }
}
pub(crate) fn http_error(error: reqwest::Error) -> io::Error {
    io::Error::other(format!(
        "SPARQL HTTP request failed: {}",
        error.without_url()
    ))
}
pub(crate) fn status(status: reqwest::StatusCode) -> io::Result<()> {
    if status.is_success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "SPARQL service returned HTTP {status}"
        )))
    }
}
