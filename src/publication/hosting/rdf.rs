//! RDF files and negotiated paths that RDF renderer targets recorded in an
//! input tree's coordination area. A routing platform serves each file with
//! its knowledge-graph media type and each negotiated path by Accept header.
use {
    super::routes::{Has, Route, Source},
    crate::publication::{
        rdf_target::{MediaType, NegotiatedPath},
        render::{PublicationManifest, RdfPublication},
    },
    anyhow::{Context, Result, ensure},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
    },
};

/// bounded: RDF target manifests read per input tree.
const MANIFESTS: usize = 256;

/// One path a host serves by the request's Accept header.
struct Negotiated {
    path:     String,
    html:     String,
    /// Packaged file per media type, in media-type order.
    variants: BTreeMap<MediaType, String>,
}

#[derive(Default)]
pub(super) struct RdfRoutes {
    /// Media type of each packaged RDF file.
    files:        BTreeMap<String, MediaType>,
    negotiations: Vec<Negotiated>,
}

fn join(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        path.to_owned()
    } else {
        format!("{prefix}/{path}")
    }
}

impl RdfRoutes {
    /// Read the RDF target manifests of every input tree packaged at its
    /// prefix, requiring each recorded file to be packaged.
    pub(super) async fn read(
        trees: &[(PathBuf, String)],
        packaged: &BTreeSet<String>,
    ) -> Result<Self> {
        let mut routes = Self::default();
        for (source, prefix) in trees {
            routes.read_tree(source, prefix, packaged).await?;
        }
        routes.negotiations.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(routes)
    }

    async fn read_tree(
        &mut self,
        source: &Path,
        prefix: &str,
        packaged: &BTreeSet<String>,
    ) -> Result<()> {
        let index = RdfPublication::index(source);
        if !tokio::fs::try_exists(&index).await? {
            return Ok(());
        }
        let mut entries = tokio::fs::read_dir(&index).await?;
        let mut names = Vec::new(); // bounded: MANIFESTS.
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().into_string().map_err(|_| {
                anyhow::anyhow!("RDF target manifest names must be UTF-8")
            })?;
            if name.ends_with(".yaml") {
                ensure!(
                    names.len() < MANIFESTS,
                    "input tree exceeds {MANIFESTS} RDF target manifests"
                );
                names.push(entry.path());
            }
        }
        names.sort();
        for path in names {
            let bytes = tokio::fs::read(&path).await?;
            let manifest: PublicationManifest = serde_yaml::from_slice(&bytes)
                .with_context(|| {
                    format!("RDF target manifest {}", path.display())
                })?;
            let rdf = manifest.rdf.with_context(|| {
                format!("{} records no RDF publication", path.display())
            })?;
            self.add(rdf, prefix, packaged)?;
        }
        Ok(())
    }

    fn add(
        &mut self,
        rdf: RdfPublication,
        prefix: &str,
        packaged: &BTreeSet<String>,
    ) -> Result<()> {
        let directory = join(prefix, rdf.destination.trim_end_matches('/'));
        let mut variants = BTreeMap::new();
        for file in rdf.files {
            let path = format!("{directory}/{}", file.name);
            ensure!(
                packaged.contains(&path),
                "RDF file {path} was not packaged"
            );
            ensure!(
                variants
                    .insert(file.media_type.clone(), path.clone())
                    .is_none() ||
                    rdf.negotiation.is_none(),
                "negotiated RDF target {directory} publishes media type {} \
                 more than once",
                file.media_type
            );
            self.files.insert(path, file.media_type);
        }
        if let Some(negotiation) = rdf.negotiation {
            let html = join(prefix, &negotiation.html);
            ensure!(
                packaged.contains(&html),
                "negotiated HTML entry {html} was not packaged"
            );
            let path = join(prefix, NegotiatedPath::as_str(&negotiation.path));
            ensure!(
                !self.negotiations.iter().any(|known| known.path == path),
                "negotiated path {path} is declared more than once"
            );
            self.negotiations.push(Negotiated { path, html, variants });
        }
        Ok(())
    }

    /// Paths a files-only platform cannot serve, named for diagnostics.
    pub(super) fn routed(&self) -> Vec<String> {
        self.negotiations
            .iter()
            .map(|negotiation| {
                format!("content negotiation of {}", negotiation.path)
            })
            .collect()
    }

    /// Routes serving negotiated paths and each file's media type.
    pub(super) fn routes(&self, url: impl Fn(&str) -> String) -> Vec<Route> {
        let mut routes = Vec::new(); // bounded: packaged files.
        for negotiation in &self.negotiations {
            let source = super::routes::exact(&url(&negotiation.path));
            for (media_type, file) in &negotiation.variants {
                routes.push(Route::Source(Source {
                    src: source.clone(),
                    dest: Some(url(file)),
                    has: vec![Has::accept(media_type)],
                    headers: BTreeMap::from([
                        ("Content-Type".into(), media_type.to_string()),
                        ("Vary".into(), "Accept".into()),
                    ]),
                    ..Source::default()
                }));
            }
            routes.push(Route::Source(Source {
                src: source,
                dest: Some(url(&negotiation.html)),
                headers: BTreeMap::from([("Vary".into(), "Accept".into())]),
                ..Source::default()
            }));
        }
        for (file, media_type) in &self.files {
            routes.push(Route::Source(Source {
                src: super::routes::exact(&url(file)),
                headers: BTreeMap::from([(
                    "Content-Type".into(),
                    media_type.to_string(),
                )]),
                continue_: true,
                ..Source::default()
            }));
        }
        routes
    }
}
