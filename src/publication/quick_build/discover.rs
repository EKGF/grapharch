use {
    crate::publication::source_walk::{SourceBudget, SourceWalk},
    anyhow::{Result, ensure},
    std::path::{Path, PathBuf},
};

pub(super) struct Inventory {
    pub root:        PathBuf,
    pub files:       Vec<PathBuf>,
    pub definitions: Option<PathBuf>,
    pub mkdocs:      Option<PathBuf>,
}
impl Inventory {
    pub async fn read(
        source: &Path,
        output: &Path,
        budget: SourceBudget,
    ) -> Result<Self> {
        let kind = tokio::fs::metadata(source).await?;
        if kind.is_file() {
            ensure!(
                supported(source),
                "unsupported input {}; use Markdown or RDF (.ttl, .rdf, .nt, \
                 .nq, .trig, .n3)",
                source.display()
            );
            return Ok(Self {
                root:        source.parent().unwrap().to_owned(),
                files:       vec![source.to_owned()],
                definitions: None,
                mkdocs:      None,
            });
        }
        ensure!(
            kind.is_dir(),
            "source must be a file or directory"
        );
        let mut definitions = None;
        for name in ["grapharch.ttl", "project.ttl", ".grapharch/project.ttl"] {
            let path = source.join(name);
            match tokio::fs::symlink_metadata(&path).await {
                Ok(metadata) => {
                    ensure!(
                        metadata.is_file(),
                        "configuration must be a regular file: {}",
                        path.display()
                    );
                    ensure!(
                        definitions.is_none(),
                        "multiple GraphArch configuration files exist; select \
                         configuration with an explicit publication command"
                    );
                    definitions = Some(path);
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                },
                Err(error) => return Err(error.into()),
            }
        }
        if definitions.is_some() {
            return Ok(Self {
                root: source.to_owned(),
                files: Vec::new(),
                definitions,
                mkdocs: None,
            });
        }
        let mut mkdocs = None;
        for name in ["mkdocs.yml", "mkdocs.yaml"] {
            let path = source.join(name);
            if tokio::fs::try_exists(&path).await? {
                ensure!(
                    mkdocs.is_none(),
                    "both mkdocs.yml and mkdocs.yaml exist; select one \
                     configuration"
                );
                ensure!(
                    tokio::fs::symlink_metadata(&path).await?.is_file(),
                    "MkDocs configuration must be a regular file"
                );
                mkdocs = Some(path);
            }
        }
        if mkdocs.is_some() {
            return Ok(Self {
                root: source.to_owned(),
                files: Vec::new(),
                definitions: None,
                mkdocs,
            });
        }
        let excluded = if output.is_absolute() {
            output.to_owned()
        } else {
            std::env::current_dir()?.join(output)
        };
        let files = SourceWalk::new(source, quick_source)
            .skipping(excluded)
            .with_budget(budget)
            .files()
            .await?;
        ensure!(
            !files.is_empty(),
            "no supported Markdown or RDF sources found in {}",
            source.display()
        );
        Ok(Self {
            root: source.to_owned(),
            files,
            definitions: None,
            mkdocs: None,
        })
    }
}
pub(super) fn rdf(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            matches!(value, "ttl" | "rdf" | "nt" | "nq" | "trig" | "n3")
        })
}
fn supported(path: &Path) -> bool {
    rdf(path) || path.extension().is_some_and(|value| value == "md")
}
fn quick_source(path: &Path) -> bool {
    supported(path) || super::super::collect::LocalImageFile::supported(path)
}
