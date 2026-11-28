//! One case per reference kind and per original form.
use {
    super::{
        Classification,
        ForgeObject,
        LinkPolicy,
        LookupPage,
        ReferenceClassifier,
        ReferenceForm,
        ReferenceKind,
        ReferenceLookup,
    },
    anyhow::Result,
    url::Url,
};

struct Site;

impl ReferenceLookup for Site {
    fn semantic(&self, value: &str) -> Result<Option<String>> {
        Ok((value == "urn:x:guide").then(|| "../guide/".to_owned()))
    }

    fn page(&self, path: &str, anchor: Option<&str>) -> Option<LookupPage> {
        let (route, asset) = match path {
            "README.md" | "" => ("", false),
            "docs/guide.md" | "docs/guide/" => ("docs/guide/", false),
            "docs/logo.png" => ("docs/logo.png", true),
            "docs/flow.puml" => ("docs/flow.puml", true),
            _ => return None,
        };
        Some(LookupPage {
            route: route.into(),
            asset,
            has_heading: anchor.is_none_or(|anchor| anchor == "install"),
        })
    }

    fn own_site(&self, url: &Url) -> Option<String> {
        url.as_str()
            .strip_prefix("https://pets.example/")
            .map(str::to_owned)
    }

    fn sibling(&self, url: &Url) -> Option<Result<String>> {
        (url.as_str() == "https://github.com/o/other")
            .then(|| Ok("https://other.example/".to_owned()))
    }

    fn repository_file(
        &self,
        _page: &str,
        link: &str,
    ) -> Option<Result<String>> {
        Some(Ok(format!(
            "https://github.com/o/r/blob/abc/{}",
            link.trim_start_matches("./")
        )))
    }

    fn collected_image(&self, url: &Url) -> bool {
        url.host_str() == Some("img.example")
    }
}

fn classify(value: &str, image: bool) -> Classification {
    ReferenceClassifier::new(&Site, LinkPolicy::Lenient)
        .classify(value, "docs/guide.md", "docs/guide/", image)
        .unwrap()
}

#[test]
fn every_original_form_is_recognized() {
    let cases: &[(&str, &str)] = &[
        ("#install", "FragmentForm"),
        ("guide.md#x", "RelativeForm"),
        ("/docs/guide/", "RootRelativeForm"),
        ("https://example.org/x", "AbsoluteForm"),
        ("//example.org/x", "SchemeRelativeForm"),
        ("mailto:a@example.org", "ContactForm"),
        ("tel:+3100", "ContactForm"),
        ("urn:isbn:1", "OtherSchemeForm"),
    ];
    for (value, term) in cases {
        assert_eq!(
            ReferenceForm::parse(value).term(),
            *term,
            "{value}"
        );
    }
}

#[test]
fn every_kind_is_decided_from_the_table() {
    let cases: &[(&str, bool, ReferenceKind, Option<&str>)] = &[
        (
            "guide.md#install",
            false,
            ReferenceKind::Page,
            Some("../../docs/guide/#install"),
        ),
        (
            "urn:x:guide",
            false,
            ReferenceKind::Page,
            Some("../guide/"),
        ),
        (
            "/docs/guide/",
            false,
            ReferenceKind::Page,
            Some("../../docs/guide/"),
        ),
        (
            "#install",
            false,
            ReferenceKind::Anchor,
            Some("#install"),
        ),
        (
            "logo.png",
            true,
            ReferenceKind::Asset,
            Some("../../docs/logo.png"),
        ),
        (
            "flow.puml",
            false,
            ReferenceKind::DiagramSource,
            Some("../../docs/flow.puml"),
        ),
        (
            "../package.json",
            false,
            ReferenceKind::RepoFile,
            Some("https://github.com/o/r/blob/abc/../package.json"),
        ),
        (
            "https://github.com/o/r/issues/4",
            false,
            ReferenceKind::ForgeObject(ForgeObject::Issue),
            None,
        ),
        (
            "https://github.com/o/r/pull/5",
            false,
            ReferenceKind::ForgeObject(ForgeObject::PullRequest),
            None,
        ),
        (
            "https://gitlab.com/g/p/-/merge_requests/1",
            false,
            ReferenceKind::ForgeObject(ForgeObject::PullRequest),
            None,
        ),
        (
            "https://github.com/o/r/commit/abc",
            false,
            ReferenceKind::ForgeObject(ForgeObject::Commit),
            None,
        ),
        (
            "https://github.com/o/r/releases/tag/v1",
            false,
            ReferenceKind::ForgeObject(ForgeObject::Release),
            None,
        ),
        (
            "https://github.com/o/r/discussions/2",
            false,
            ReferenceKind::ForgeObject(ForgeObject::Discussion),
            None,
        ),
        (
            "https://github.com/o/other",
            false,
            ReferenceKind::SiblingSite,
            Some("https://other.example/"),
        ),
        (
            "https://pets.example/docs/guide/",
            false,
            ReferenceKind::OwnSiteAbsolute,
            Some("../../docs/guide/"),
        ),
        (
            "https://img.shields.io/crates/v/x.svg",
            true,
            ReferenceKind::Badge,
            None,
        ),
        (
            "https://docs.rs/x/badge.svg",
            true,
            ReferenceKind::Badge,
            None,
        ),
        (
            "https://img.example/a.png",
            true,
            ReferenceKind::Asset,
            None,
        ),
        (
            "https://example.org/x",
            false,
            ReferenceKind::External,
            None,
        ),
        (
            "//example.org/x",
            false,
            ReferenceKind::External,
            Some("https://example.org/x"),
        ),
        (
            "mailto:a@example.org",
            false,
            ReferenceKind::External,
            None,
        ),
    ];
    for (value, image, kind, href) in cases {
        let classification = classify(value, *image);
        assert_eq!(&classification.kind, kind, "{value}");
        if let Some(href) = href {
            assert_eq!(
                classification.href.as_deref(),
                Some(*href),
                "{value}"
            );
        }
    }
}

#[test]
fn broken_references_are_text_unless_strict() {
    for value in ["#missing", "guide.md#missing", "/nowhere/", "javascript:x"] {
        let classification = classify(value, false);
        assert!(
            matches!(classification.kind, ReferenceKind::Broken(_)),
            "{value}"
        );
        assert!(classification.href.is_none());
        assert!(
            ReferenceClassifier::new(&Site, LinkPolicy::Strict)
                .classify(value, "docs/guide.md", "docs/guide/", false)
                .is_err(),
            "{value}"
        );
    }
}
