//! The public locations of the website itself.
//!
//! A website target may name where this build is published (`ga:thisVersion`)
//! and the stable location of its latest version (`ga:latestVersion`). Those
//! IRIs are metadata: canonical links, Open Graph and the sitemap. A link
//! that an author, a footer or a generated page writes as an absolute URL
//! beneath either location still points into this website, so it renders as a
//! path relative to the linking page. The output therefore works wherever it
//! is deployed and never sends a reader of a preview or a mirror to another
//! host.
use {
    super::super::model::Plan,
    anyhow::{Result, bail, ensure},
    oxrdf::Term,
    url::Url,
};

/// Website location properties, in the order they are read.
const LOCATIONS: [&str; 2] = ["thisVersion", "latestVersion"];

/// The declared public directory URLs of the website.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SiteBases(
    // bounded: at most one URL per entry of LOCATIONS.
    Vec<Url>,
);

impl SiteBases {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let mut bases = Vec::new();
        for property in LOCATIONS {
            match plan.source.optional(&plan.target, property).await? {
                None => {},
                Some(Term::NamedNode(value)) => {
                    bases.push(Self::directory(value.as_str(), property)?);
                },
                Some(_) => bail!("website {property} must be an IRI"),
            }
        }
        Ok(Self(bases))
    }

    /// Validates one declared location: an absolute HTTP(S) directory URL.
    fn directory(value: &str, property: &str) -> Result<Url> {
        let url = Url::parse(value)?;
        ensure!(
            matches!(url.scheme(), "https" | "http") &&
                url.host_str().is_some() &&
                url.username().is_empty() &&
                url.password().is_none() &&
                url.query().is_none() &&
                url.fragment().is_none() &&
                url.path().ends_with('/'),
            "website {property} must be an absolute HTTP(S) directory IRI \
             without credentials/query/fragment"
        );
        Ok(url)
    }

    #[cfg(test)]
    pub fn try_new(values: &[&str]) -> Result<Self> {
        values
            .iter()
            .map(|value| Self::directory(value, "location"))
            .collect::<Result<_>>()
            .map(Self)
    }

    /// The website-root-relative form of `url` when it lies beneath one of
    /// the website's locations, such as `spec-versions/` or `spec/#top`. The
    /// location itself yields an empty path. Scheme and host compare as URLs
    /// do; the path compares by whole segments.
    pub fn localize(&self, url: &Url) -> Option<String> {
        self.0.iter().find_map(|base| {
            if url.scheme() != base.scheme() ||
                url.host_str() != base.host_str() ||
                url.port_or_known_default() != base.port_or_known_default() ||
                !url.username().is_empty() ||
                url.password().is_some()
            {
                return None;
            }
            let path = if url.path() == base.path().trim_end_matches('/') {
                ""
            } else {
                url.path().strip_prefix(base.path())?
            };
            let mut local = path.to_owned();
            if let Some(query) = url.query() {
                local.push('?');
                local.push_str(query);
            }
            if let Some(fragment) = url.fragment() {
                local.push('#');
                local.push_str(fragment);
            }
            Some(local)
        })
    }
}

#[cfg(test)]
mod tests {
    use {super::SiteBases, url::Url};

    fn localize(bases: &SiteBases, value: &str) -> Option<String> {
        bases.localize(&Url::parse(value).unwrap())
    }

    #[test]
    fn links_beneath_either_location_become_website_paths() {
        let bases = SiteBases::try_new(&[
            "https://example.github.io/project/",
            "https://example.org/project/",
        ])
        .unwrap();
        for (value, local) in [
            ("https://example.org/project/", ""),
            ("https://example.org/project", ""),
            (
                "https://example.org/project/versions/",
                "versions/",
            ),
            (
                "https://example.org/project/spec/#shape",
                "spec/#shape",
            ),
            ("https://example.org:443/project/a?b=c", "a?b=c"),
            (
                "https://example.github.io/project/spec/main/",
                "spec/main/",
            ),
        ] {
            assert_eq!(
                localize(&bases, value).as_deref(),
                Some(local),
                "{value}"
            );
        }
    }

    #[test]
    fn other_locations_remain_absolute() {
        let bases =
            SiteBases::try_new(&["https://example.org/project/"]).unwrap();
        for value in [
            "https://example.org/",
            "https://example.org/about/",
            "https://example.org/projects/",
            "https://example.org/project-two/",
            "http://example.org/project/",
            "https://example.org:8443/project/",
            "https://other.example.org/project/",
            "https://user@example.org/project/",
        ] {
            assert_eq!(localize(&bases, value), None, "{value}");
        }
        assert_eq!(
            localize(&SiteBases::default(), "https://a.b/"),
            None
        );
    }

    #[test]
    fn locations_must_be_directories() {
        for value in [
            "https://example.org/project",
            "https://example.org/project/?a",
            "https://example.org/project/#a",
            "ftp://example.org/project/",
        ] {
            assert!(SiteBases::try_new(&[value]).is_err(), "{value}");
        }
    }
}
