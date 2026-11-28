//! The documented software project, read from the repository facts in the
//! pinned graph: the version chip and the home page's project facts.
use {
    super::super::{markup::html_escape as esc, model::Plan},
    anyhow::Result,
    oxrdf::{NamedNode, Term},
};

const DOAP: &str = "http://usefulinc.com/ns/doap#";
const SCHEMA: &str = "https://schema.org/";
/// Most registry or reference links shown.
const MAX_LINKS: usize = 16;

fn predicate(namespace: &str, local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{namespace}{local}"))
}

/// One labelled link of the project facts.
struct FactLink {
    label: String,
    url:   String,
}

/// What the website shows about the documented software project.
#[derive(Default)]
pub(super) struct Software {
    version:  Option<String>,
    license:  Option<(String, Option<String>)>,
    links:    Vec<FactLink>, // bounded: MAX_LINKS.
    /// Workspace packages with their own versions, when there is no single
    /// project version.
    packages: Vec<PackageFact>, // bounded: MAX_PACKAGES.
}

/// Most workspace packages listed on the home page.
const MAX_PACKAGES: usize = 64;

/// One package of a workspace: name, version and registry page.
struct PackageFact {
    name:     String,
    version:  Option<String>,
    registry: Option<String>,
}

impl Software {
    pub(super) async fn read(plan: &Plan) -> Result<Option<Self>> {
        let view = &plan.source;
        let Some(Term::NamedNode(project)) =
            view.optional(&plan.project, "softwareProject").await?
        else {
            return Ok(None);
        };
        let literal = |term: Option<Term>| {
            match term {
                Some(Term::Literal(value)) => Some(value.value().to_owned()),
                _ => None,
            }
        };
        let iri = |term: Option<Term>| {
            match term {
                Some(Term::NamedNode(node)) => Some(node.into_string()),
                _ => None,
            }
        };
        let mut software = Self {
            version: literal(
                view.optional_by(
                    &project,
                    &predicate(SCHEMA, "softwareVersion"),
                )
                .await?,
            ),
            ..Self::default()
        };
        if let Some(Term::NamedNode(repository)) = view
            .optional_by(&project, &predicate(DOAP, "repository"))
            .await? &&
            software.version.is_none()
        {
            software.version =
                literal(view.optional(&repository, "latestTag").await?);
        }
        if software.version.is_none() {
            software.packages = Self::packages(view, &project).await?;
        }
        if let Some(expression) =
            literal(view.optional(&project, "licenseExpression").await?)
        {
            let url = iri(view
                .optional_by(&project, &predicate(DOAP, "license"))
                .await?);
            software.license = Some((expression, url));
        }
        if let Some(Term::NamedNode(repository)) = view
            .optional_by(&project, &predicate(DOAP, "repository"))
            .await? &&
            let Some(url) = iri(view
                .optional_by(&repository, &predicate(DOAP, "browse"))
                .await?)
        {
            software.push("Repository", url);
        }
        if let Some(url) = iri(view
            .optional_by(&project, &predicate(DOAP, "bug-database"))
            .await?)
        {
            software.push("Issues", url);
        }
        if let Some(url) = iri(view
            .optional_by(&project, &predicate(SCHEMA, "discussionUrl"))
            .await?)
        {
            software.push("Discussions", url);
        }
        if let Some(Term::NamedNode(release)) = view
            .optional_by(&project, &predicate(DOAP, "release"))
            .await? &&
            let Some(url) =
                iri(view.optional(&release, "releaseNotes").await?)
        {
            let tag = literal(
                view.optional_by(&release, &predicate(DOAP, "revision"))
                    .await?,
            );
            software.push(
                &format!("Release {}", tag.unwrap_or_default()),
                url,
            );
        }
        if let Some(url) = iri(view
            .optional_by(&project, &predicate(DOAP, "homepage"))
            .await?)
        {
            software.push("Homepage", url);
        }
        if let Some(Term::NamedNode(package)) =
            view.optional(&project, "primaryPackage").await?
        {
            for (label, property) in [
                ("Registry", "registryPage"),
                ("API reference", "apiReference"),
            ] {
                if let Some(url) =
                    iri(view.optional(&package, property).await?)
                {
                    software.push(&registry_label(label, &url), url);
                }
            }
        }
        Ok(Some(software))
    }

    async fn packages(
        view: &crate::publication::view::RevisionView,
        project: &NamedNode,
    ) -> Result<Vec<PackageFact>> {
        let mut packages = Vec::new(); // bounded: MAX_PACKAGES.
        let mut rows = view.objects(project, "codePackage").await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(package) = &row["value"] else {
                continue
            };
            if packages.len() == MAX_PACKAGES {
                break;
            }
            let text = |term: Option<Term>| {
                match term {
                    Some(Term::Literal(value)) => {
                        Some(value.value().to_owned())
                    },
                    Some(Term::NamedNode(node)) => Some(node.into_string()),
                    _ => None,
                }
            };
            let Some(name) = text(
                view.optional_by(package, &predicate(DOAP, "name")).await?,
            ) else {
                continue;
            };
            packages.push(PackageFact {
                name,
                version: text(
                    view.optional_by(
                        package,
                        &predicate(SCHEMA, "softwareVersion"),
                    )
                    .await?,
                ),
                registry: text(view.optional(package, "registryPage").await?),
            });
        }
        packages.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(packages)
    }

    fn push(&mut self, label: &str, url: String) {
        if self.links.len() < MAX_LINKS &&
            !self.links.iter().any(|link| link.url == url)
        {
            self.links
                .push(FactLink { label: label.trim().to_owned(), url });
        }
    }

    /// The package version for the header chip, such as `v1.4.0`.
    pub(super) fn version(&self) -> Option<&str> { self.version.as_deref() }

    /// The home page's project facts: version, license and links.
    pub(super) fn facts_html(&self) -> String {
        let mut items = String::new();
        if let Some(version) = &self.version {
            items.push_str(&format!(
                "<div><dt>Version</dt><dd>{}</dd></div>",
                esc(version)
            ));
        }
        if let Some((expression, url)) = &self.license {
            let value = match url {
                Some(url) => {
                    format!("<a href=\"{}\">{}</a>", esc(url), esc(expression))
                },
                None => esc(expression).to_string(),
            };
            items.push_str(&format!(
                "<div><dt>License</dt><dd>{value}</dd></div>"
            ));
        }
        if !self.links.is_empty() {
            let links = self
                .links
                .iter()
                .map(|link| {
                    format!(
                        "<a href=\"{}\">{}</a>",
                        esc(&link.url),
                        esc(&link.label)
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
            items.push_str(&format!(
                "<div><dt>Links</dt><dd>{links}</dd></div>"
            ));
        }
        if !self.packages.is_empty() {
            let packages = self
                .packages
                .iter()
                .map(|package| {
                    let name = match &package.registry {
                        Some(url) => {
                            format!(
                                "<a href=\"{}\">{}</a>",
                                esc(url),
                                esc(&package.name)
                            )
                        },
                        None => esc(&package.name).to_string(),
                    };
                    match &package.version {
                        Some(version) => {
                            format!(
                                "{name} <span \
                                 class=\"package-version\">{}</span>",
                                esc(version)
                            )
                        },
                        None => name,
                    }
                })
                .collect::<Vec<_>>()
                .join(" · ");
            items.push_str(&format!(
                "<div><dt>Packages</dt><dd>{packages}</dd></div>"
            ));
        }
        if items.is_empty() {
            return String::new();
        }
        format!(
            "<dl class=\"project-facts\" aria-label=\"Project \
             facts\">{items}</dl>"
        )
    }
}

/// Name a registry link by its host, such as `crates.io` or `docs.rs`.
fn registry_label(fallback: &str, url: &str) -> String {
    match url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
    {
        Some(host) => host.trim_start_matches("www.").to_owned(),
        None => fallback.to_owned(),
    }
}
