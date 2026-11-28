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
/// Converts a transport failure into an I/O error that names every cause.
///
/// `reqwest` reports connect failures, timeouts and dropped connections all as
/// "error sending request"; the distinguishing cause is only in the source
/// chain, so the chain is rendered rather than discarded.
pub(crate) fn http_error(error: reqwest::Error) -> io::Error {
    let error = error.without_url();
    let mut message = format!("SPARQL HTTP request failed: {error}");
    let mut cause = std::error::Error::source(&error);
    while let Some(inner) = cause {
        message.push_str(": ");
        message.push_str(&inner.to_string());
        cause = inner.source();
    }
    if error.is_timeout() {
        message.push_str(" (timed out)");
    }
    io::Error::other(message)
}
/// Upper bound on the body bytes read and discarded to return a connection to
/// the client's pool; a larger remainder closes the connection instead.
const DRAIN_LIMIT: u64 = 64 * 1024;
/// Reads what remains of a response body to its end, so `reqwest` returns the
/// connection to its pool instead of closing it.
///
/// A response dropped with unread body bytes closes its connection; on a
/// long publication every such close leaves a TIME_WAIT socket behind, and on
/// Windows those exhaust the dynamic port range (`os error 10048`).
pub(crate) trait Drain: Read + Sized {
    /// Discards the remaining body, at most [`DRAIN_LIMIT`] bytes.
    fn drain(&mut self) -> io::Result<()> {
        io::copy(
            &mut self.by_ref().take(DRAIN_LIMIT),
            &mut io::sink(),
        )
        .map(|_| ())
    }
}
impl Drain for reqwest::blocking::Response {}
impl<R: Read> Drain for LimitedRead<R> {}
/// A response whose body carries nothing the caller reads.
pub(crate) trait Settle {
    /// Discards the body, then reports the status like [`status`].
    fn settle(self) -> io::Result<()>;
}
impl Settle for reqwest::blocking::Response {
    fn settle(mut self) -> io::Result<()> {
        let drained = self.drain();
        status(self.status())?;
        drained
    }
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
