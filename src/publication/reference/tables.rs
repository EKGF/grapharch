//! The data tables the classifier consults.
use {super::kind::ForgeObject, url::Url};

/// Hosts that serve status badges; images from them are never fetched.
const BADGE_HOSTS: &[&str] = &[
    "img.shields.io",
    "shields.io",
    "badge.fury.io",
    "badgen.net",
    "flat.badgen.net",
    "codecov.io",
    "coveralls.io",
    "deps.rs",
    "docs.rs",
    "img.badgesize.io",
    "app.codacy.com",
    "sonarcloud.io",
    "api.codeclimate.com",
    "results.pre-commit.ci",
    "readthedocs.org",
    "github.com",
    "gitlab.com",
    "pepy.tech",
    "static.pepy.tech",
    "snyk.io",
];

/// Path fragments that mark an image on a badge host as a badge.
const BADGE_PATHS: &[(&str, &str)] = &[
    ("github.com", "/badge.svg"),
    ("gitlab.com", "/badges/"),
    ("docs.rs", "/badge.svg"),
    ("codecov.io", "/graph/badge.svg"),
    ("readthedocs.org", "/badge/"),
];

/// Whether an image URL is a status badge.
pub(crate) fn badge(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false
    };
    if !BADGE_HOSTS.contains(&host) {
        return false;
    }
    match BADGE_PATHS
        .iter()
        .find(|(badge_host, _)| *badge_host == host)
    {
        Some((_, path)) => url.path().contains(path),
        None => true,
    }
}

/// Forge URL path segments, after `owner/repo`, that name a forge object.
const FORGE_OBJECTS: &[(&str, ForgeObject)] = &[
    ("issues", ForgeObject::Issue),
    ("pull", ForgeObject::PullRequest),
    ("merge_requests", ForgeObject::PullRequest),
    ("commit", ForgeObject::Commit),
    ("releases", ForgeObject::Release),
    ("discussions", ForgeObject::Discussion),
];

/// The forge object a GitHub or GitLab URL names, if any.
pub(super) fn forge_object(url: &Url) -> Option<ForgeObject> {
    let host = url.host_str()?;
    if !matches!(host, "github.com" | "gitlab.com") {
        return None;
    }
    let segments: Vec<&str> =
        url.path_segments()?.filter(|s| !s.is_empty()).collect(); // bounded: one URL path.
    let rest = match segments.iter().position(|s| *s == "-") {
        Some(at) => &segments[at + 1..],
        None if segments.len() > 2 => &segments[2..],
        None => return None,
    };
    let first = rest.first()?;
    FORGE_OBJECTS
        .iter()
        .find(|(segment, _)| segment == first)
        .map(|(_, object)| *object)
}

/// Fenced-code languages that render as diagrams.
pub(super) const DIAGRAM_EXTENSIONS: &[&str] =
    &["puml", "plantuml", "mmd", "mermaid", "dot", "gv"];
