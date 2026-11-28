//! The pure classification of one reference against a publication lookup.
use {
    super::{
        form::ReferenceForm,
        kind::{Classification, ReferenceKind},
        tables,
    },
    anyhow::{Result, bail},
    url::Url,
};

/// Whether an unresolved reference fails the build.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LinkPolicy {
    /// Fail on the first broken reference (configured publications).
    #[default]
    Strict,
    /// Render broken references as text with a diagnostic (zero
    /// configuration, unless `--strict`).
    Lenient,
}

/// Project property selecting the link policy.
pub(crate) const POLICY_SETTING: &str = "linkPolicy";

impl LinkPolicy {
    /// The policy `project` declares: `ga:LenientLinks` or the default
    /// `ga:StrictLinks`.
    pub(crate) async fn read(
        view: &crate::publication::view::RevisionView,
        project: &oxrdf::NamedNode,
    ) -> Result<Self> {
        match view.optional(project, POLICY_SETTING).await? {
            None => Ok(Self::Strict),
            Some(oxrdf::Term::NamedNode(node))
                if node == crate::publication::term("LenientLinks") =>
            {
                Ok(Self::Lenient)
            },
            Some(oxrdf::Term::NamedNode(node))
                if node == crate::publication::term("StrictLinks") =>
            {
                Ok(Self::Strict)
            },
            Some(other) => {
                bail!(
                    "ga:{POLICY_SETTING} must be ga:StrictLinks or \
                     ga:LenientLinks, not {other}"
                )
            },
        }
    }
}

impl LinkPolicy {
    /// The policy's individual in the GraphArch vocabulary.
    pub(crate) fn term(self) -> &'static str {
        match self {
            Self::Strict => "StrictLinks",
            Self::Lenient => "LenientLinks",
        }
    }
}

/// A page of the publication, as the lookup reports it.
pub(crate) struct LookupPage {
    /// Route relative to the site root, such as `guide/`.
    pub(crate) route:       String,
    pub(crate) asset:       bool,
    pub(crate) has_heading: bool,
}

/// The publication facts the classifier decides against.
pub(crate) trait ReferenceLookup {
    /// A semantic link by IRI (target, page, subject or collected image),
    /// already relative to the referring page.
    fn semantic(&self, value: &str) -> Result<Option<String>>;
    /// The page or asset whose documentation-root-relative source or route is
    /// `path`; `anchor` is checked against its headings.
    fn page(&self, path: &str, anchor: Option<&str>) -> Option<LookupPage>;
    /// A local route for a URL beneath this website's own location.
    fn own_site(&self, url: &Url) -> Option<String>;
    /// A sibling website's URL for a repository URL, or a local source.
    fn sibling(&self, url: &Url) -> Option<Result<String>>;
    /// The forge blob URL of a repository file the page links to.
    fn repository_file(&self, page: &str, link: &str)
    -> Option<Result<String>>;
    /// Whether `url` is an image of this page that was collected, not fetched.
    fn collected_image(&self, url: &Url) -> bool;
}

/// Decides the kind of each reference; never performs I/O.
pub(crate) struct ReferenceClassifier<'a, L: ReferenceLookup> {
    lookup: &'a L,
    policy: LinkPolicy,
}

impl<'a, L: ReferenceLookup> ReferenceClassifier<'a, L> {
    pub(crate) fn new(lookup: &'a L, policy: LinkPolicy) -> Self {
        Self { lookup, policy }
    }

    /// Classify `value`, written in the page with source `source` and route
    /// `route`. An error is returned only for a broken reference under
    /// [`LinkPolicy::Strict`].
    pub(crate) fn classify(
        &self,
        value: &str,
        source: &str,
        route: &str,
        image: bool,
    ) -> Result<Classification> {
        if let Some(href) = self.lookup.semantic(value)? {
            return Ok(Classification::new(ReferenceKind::Page, href));
        }
        let prefix = "../".repeat(route.matches('/').count());
        let classification = match ReferenceForm::parse(value) {
            ReferenceForm::FragmentOnly(anchor) => self.anchor(&anchor, source),
            ReferenceForm::Contact(url) => {
                Classification::external(ReferenceKind::External, &url)
            },
            ReferenceForm::OtherScheme(scheme) => {
                Classification::broken(format!(
                    "unsupported {scheme}: reference {value}"
                ))
            },
            ReferenceForm::Absolute(url) |
            ReferenceForm::SchemeRelative(url) => {
                self.absolute(&url, &prefix, image)?
            },
            ReferenceForm::Relative { path, fragment } => {
                self.local(
                    value,
                    &path,
                    fragment.as_deref(),
                    source,
                    &prefix,
                    false,
                )?
            },
            ReferenceForm::RootRelative { path, fragment } => {
                self.local(
                    value,
                    &path,
                    fragment.as_deref(),
                    source,
                    &prefix,
                    true,
                )?
            },
        };
        if let ReferenceKind::Broken(reason) = &classification.kind &&
            self.policy == LinkPolicy::Strict
        {
            bail!("{source}: {reason}")
        }
        Ok(classification)
    }

    fn anchor(&self, anchor: &str, source: &str) -> Classification {
        if anchor.is_empty() ||
            self.lookup
                .page(source, Some(anchor))
                .is_some_and(|page| page.has_heading)
        {
            Classification::new(ReferenceKind::Anchor, format!("#{anchor}"))
        } else {
            Classification::broken(format!(
                "unresolved heading #{anchor} in {source}"
            ))
        }
    }

    fn absolute(
        &self,
        url: &Url,
        prefix: &str,
        image: bool,
    ) -> Result<Classification> {
        if image && tables::badge(url) {
            return Ok(Classification::external(
                ReferenceKind::Badge,
                url,
            ));
        }
        if image && self.lookup.collected_image(url) {
            return Ok(Classification::external(
                ReferenceKind::Asset,
                url,
            ));
        }
        if let Some(local) = self.lookup.own_site(url) {
            return Ok(Classification::new(
                ReferenceKind::OwnSiteAbsolute,
                if prefix.is_empty() && local.is_empty() {
                    "./".to_owned()
                } else {
                    format!("{prefix}{local}")
                },
            ));
        }
        if let Some(href) = self.lookup.sibling(url) {
            return Ok(Classification::new(
                ReferenceKind::SiblingSite,
                href?,
            ));
        }
        if let Some(object) = tables::forge_object(url) {
            return Ok(Classification::external(
                ReferenceKind::ForgeObject(object),
                url,
            ));
        }
        Ok(Classification::external(
            ReferenceKind::External,
            url,
        ))
    }

    fn local(
        &self,
        value: &str,
        path: &str,
        fragment: Option<&str>,
        source: &str,
        prefix: &str,
        root: bool,
    ) -> Result<Classification> {
        let base = if root {
            Url::parse("https://grapharch.invalid/")?
        } else {
            Url::parse(&format!("https://grapharch.invalid/{source}"))?
        };
        let Ok(resolved) = base.join(value.trim_start_matches('/')) else {
            return Ok(Classification::broken(format!(
                "invalid reference {value}"
            )));
        };
        if resolved.host_str() != Some("grapharch.invalid") ||
            resolved.query().is_some()
        {
            return Ok(Classification::broken(format!(
                "unsupported local reference {value}"
            )));
        }
        let target = resolved.path().trim_start_matches('/');
        if path.is_empty() {
            return Ok(self.anchor(fragment.unwrap_or_default(), source));
        }
        if let Some(page) = self.lookup.page(target, fragment) {
            if fragment.is_some_and(|anchor| !anchor.is_empty()) &&
                !page.has_heading
            {
                return Ok(Classification::broken(format!(
                    "unresolved heading {value} in {target}"
                )));
            }
            let anchor = fragment.map(|a| format!("#{a}")).unwrap_or_default();
            let kind = if page.asset {
                if tables::DIAGRAM_EXTENSIONS
                    .iter()
                    .any(|ext| target.ends_with(&format!(".{ext}")))
                {
                    ReferenceKind::DiagramSource
                } else {
                    ReferenceKind::Asset
                }
            } else {
                ReferenceKind::Page
            };
            return Ok(Classification::new(
                kind,
                format!("{prefix}{}{anchor}", page.route),
            ));
        }
        if !root && let Some(href) = self.lookup.repository_file(source, value)
        {
            return Ok(match href {
                Ok(href) => Classification::new(ReferenceKind::RepoFile, href),
                Err(error) => Classification::broken(format!("{error:#}")),
            });
        }
        Ok(Classification::broken(format!(
            "unresolved documentation link {value} (source {target}); declare \
             ga:sourceLinkBase on the project or build from a repository with \
             a forge remote to link repository files that are not pages"
        )))
    }
}
