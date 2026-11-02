use {
    super::Configuration,
    anyhow::{Context, Result, ensure},
    serde::{Deserialize, Serialize},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::Path,
    },
};

#[derive(Serialize)]
pub(super) struct Output {
    version: u8,
    routes:  Vec<Route>,
}
#[derive(Serialize)]
#[serde(untagged)]
enum Route {
    Source(Source),
    Handler { handle: &'static str },
}
#[derive(Default, Serialize)]
struct Source {
    src:     String,
    #[serde(skip_serializing_if = "Option::is_none")]
    dest:    Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    headers: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status:  Option<u16>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    observed_at: String,
    versions:    Vec<Version>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Version {
    slug:       String,
    branch:     String,
    label:      String,
    url:        String,
    advertised: bool,
}

fn url(mount: &str, path: &str) -> String {
    format!(
        "/{}{}",
        if mount.is_empty() {
            String::new()
        } else {
            format!("{mount}/")
        },
        path
    )
}
fn exact(path: &str) -> String { format!("^{}$", regex::escape(path)) }
fn rewrite(path: String, destination: String) -> Route {
    Route::Source(Source {
        src: exact(&path),
        dest: Some(destination),
        ..Source::default()
    })
}
fn redirect(path: String, destination: String, status: u16) -> Route {
    Route::Source(Source {
        src: exact(&path),
        headers: BTreeMap::from([("Location".into(), destination)]),
        status: Some(status),
        ..Source::default()
    })
}

pub(super) async fn configuration(
    settings: &Configuration,
    parent: &Path,
    mount: &str,
    files: &BTreeSet<String>,
) -> Result<Output> {
    // bounded: 4096 copied files and at most 1000 explicitly observed versions.
    let mut routes = Vec::new();
    for alias in &settings.file_aliases {
        let trailing = alias.path.ends_with('/');
        let path = super::relative(alias.path.trim_end_matches('/'))?;
        let target = super::relative(&alias.target)?;
        ensure!(
            !path.is_empty() && !target.is_empty(),
            "file aliases must name nonempty paths"
        );
        ensure!(
            files.contains(&target),
            "file alias target was not packaged: {target}"
        );
        ensure!(
            !alias.content_type.is_empty() &&
                !alias.content_type.contains(['\r', '\n']),
            "invalid artifact content type"
        );
        let path = url(mount, &path);
        let canonical = format!("{path}{}", if trailing { "/" } else { "" });
        routes.push(redirect(
            format!("{path}{}", if trailing { "" } else { "/" }),
            canonical.clone(),
            308,
        ));
        routes.push(Route::Source(Source {
            src: exact(&canonical),
            dest: Some(url(mount, &target)),
            headers: BTreeMap::from([(
                "Content-Type".into(),
                alias.content_type.clone(),
            )]),
            ..Source::default()
        }));
    }
    for file in files {
        if let Some(directory) = file.strip_suffix("index.html") {
            let canonical = url(mount, directory);
            let without = canonical.trim_end_matches('/');
            if !without.is_empty() {
                routes.push(redirect(without.into(), canonical.clone(), 308));
            }
            routes.push(rewrite(canonical, url(mount, file)));
        }
    }
    routes.push(Route::Handler { handle: "filesystem" });
    if let Some(path) = &settings.inventory {
        let version_path = super::relative(
            settings
                .version_path
                .as_deref()
                .context("inventory requires version_path")?,
        )?;
        ensure!(
            !version_path.is_empty(),
            "version_path must be nonempty"
        );
        let path = parent.join(path);
        ensure!(
            tokio::fs::metadata(&path).await?.len() <= 4 * 1024 * 1024,
            "version inventory exceeds 4 MiB"
        );
        let inventory: Inventory =
            serde_json::from_slice(&tokio::fs::read(path).await?)?;
        chrono::DateTime::parse_from_rfc3339(&inventory.observed_at)
            .context("version inventory needs an observation timestamp")?;
        ensure!(
            inventory.versions.len() <= 1000,
            "version inventory exceeds 1000 versions"
        );
        let mut seen = BTreeSet::new();
        for version in inventory.versions {
            let slug = super::relative(&version.slug)?;
            ensure!(
                !slug.is_empty() &&
                    !slug.contains('/') &&
                    seen.insert(slug.clone()),
                "version slugs must be distinct URL segments"
            );
            ensure!(
                !version.branch.is_empty() && !version.label.is_empty(),
                "version identity is incomplete"
            );
            let _advertised = version.advertised; // Routing deliberately includes unadvertised previews.
            let base = url::Url::parse(&version.url)?;
            ensure!(
                matches!(base.scheme(), "http" | "https") &&
                    base.username().is_empty() &&
                    base.password().is_none() &&
                    base.query().is_none() &&
                    base.fragment().is_none() &&
                    base.path().ends_with('/') &&
                    !version.url.contains('$'),
                "version URL must be an HTTP(S) directory without \
                 credentials/query/fragment"
            );
            // A locally materialized version always takes precedence.
            let path = format!("{version_path}/{slug}/");
            if files.contains(&format!("{path}index.html")) {
                continue;
            }
            let source = url(mount, &path);
            routes.push(redirect(
                source.trim_end_matches('/').into(),
                source.clone(),
                308,
            ));
            routes.push(Route::Source(Source {
                src: format!("^{}(.*)$", regex::escape(&source)),
                headers: BTreeMap::from([(
                    "Location".into(),
                    format!("{}$1", version.url),
                )]),
                status: Some(307),
                ..Source::default()
            }));
        }
    }
    if files.contains("404.html") {
        routes.push(Route::Source(Source {
            src: "^/.*$".into(),
            dest: Some(url(mount, "404.html")),
            status: Some(404),
            ..Source::default()
        }));
    }
    Ok(Output { version: 3, routes })
}
