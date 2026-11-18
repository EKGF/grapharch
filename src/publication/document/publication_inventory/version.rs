//! One published version listed by a publication inventory.
use {
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Version {
    pub slug:        String,
    pub branch:      String,
    pub label:       String,
    pub url:         String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route:       Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status:      Option<String>,
    /// A one-line reader description of this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The bundled icon and its tone marking this version's status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon:        Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone:        Option<String>,
    pub advertised:  bool,
}

impl Version {
    /// Where this version is served: its host route, else its URL.
    pub fn location(&self) -> &str {
        self.route.as_deref().unwrap_or(&self.url)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.slug.is_empty() &&
                self.slug.len() <= 128 &&
                self.slug != "." &&
                self.slug != ".." &&
                self.slug.bytes().all(|c| {
                    c.is_ascii_alphanumeric() || b"-_.".contains(&c)
                }),
            "invalid publication slug"
        );
        ensure!(
            !self.branch.is_empty() &&
                self.branch.len() <= 256 &&
                !self.branch.chars().any(char::is_control),
            "invalid publication branch"
        );
        ensure!(
            !self.label.trim().is_empty() &&
                self.label.len() <= 512 &&
                !self.label.chars().any(char::is_control),
            "invalid publication label"
        );
        if let Some(status) = &self.status {
            ensure!(
                !status.trim().is_empty() &&
                    status.len() <= 512 &&
                    !status.chars().any(char::is_control),
                "invalid publication status"
            );
        }
        if let Some(description) = &self.description {
            ensure!(
                !description.trim().is_empty() &&
                    description.len() <= 1024 &&
                    !description.chars().any(char::is_control),
                "invalid publication description"
            );
        }
        ensure!(
            self.icon.is_some() == self.tone.is_some(),
            "a publication icon and its tone are declared together"
        );
        if let Some(icon) = &self.icon {
            ensure!(
                !icon.is_empty() &&
                    icon.len() <= 64 &&
                    icon.bytes()
                        .all(|c| c.is_ascii_lowercase() || c == b'-'),
                "invalid publication icon name"
            );
        }
        if let Some(tone) = &self.tone {
            ensure!(
                matches!(tone.as_str(), "primary" | "accent" | "muted"),
                "publication tone must be primary, accent or muted"
            );
        }
        let url = url::Url::parse(&self.url)?;
        ensure!(
            matches!(url.scheme(), "http" | "https") &&
                url.host_str().is_some() &&
                url.username().is_empty() &&
                url.password().is_none() &&
                url.query().is_none() &&
                url.fragment().is_none() &&
                self.url.ends_with('/'),
            "publication URL must be a credential-free HTTP(S) directory URL"
        );
        if let Some(route) = &self.route {
            ensure!(
                route.starts_with('/') &&
                    route.ends_with('/') &&
                    route.len() > 2 &&
                    route.len() <= 512 &&
                    !route.starts_with("//") &&
                    route[1..route.len() - 1].split('/').all(|segment| {
                        !segment.is_empty() &&
                            segment != "." &&
                            segment != ".." &&
                            segment.bytes().all(|c| {
                                c.is_ascii_alphanumeric() || b"-_.".contains(&c)
                            })
                    }),
                "publication route must be a safe host-root directory path"
            );
        }
        Ok(())
    }
}
