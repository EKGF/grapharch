use {
    anyhow::{Result, ensure},
    std::path::{Path, PathBuf},
};

pub(super) struct Inventory {
    pub root:        PathBuf,
    pub files:       Vec<PathBuf>,
    pub definitions: Option<PathBuf>,
}
impl Inventory {
    pub async fn read(source: &Path, output: &Path) -> Result<Self> {
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
            });
        }
        ensure!(
            kind.is_dir(),
            "source must be a file or directory"
        );
        let mut definitions = None;
        for name in ["grapharch.ttl", "project.ttl"] {
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
                        "both grapharch.ttl and project.ttl exist; select \
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
            });
        }
        let excluded = if output.is_absolute() {
            output.to_owned()
        } else {
            std::env::current_dir()?.join(output)
        };
        let mut pending = vec![source.to_owned()]; // bounded: total discovery budget is 4096 entries.
        let mut files = Vec::new(); // bounded: at most 256 sources.
        let mut count = 0;
        while let Some(directory) = pending.pop() {
            let mut entries = tokio::fs::read_dir(directory).await?;
            while let Some(entry) = entries.next_entry().await? {
                count += 1;
                ensure!(
                    count <= 4096,
                    "quick build discovery exceeds 4096 entries; select a \
                     smaller source directory"
                );
                let path = entry.path();
                let name = entry.file_name();
                let name = name.to_str().ok_or_else(|| {
                    anyhow::anyhow!("source names must be UTF-8")
                })?;
                if path == excluded ||
                    name.starts_with('.') ||
                    name.starts_with('_') ||
                    matches!(
                        name,
                        "node_modules" |
                            "target" |
                            "dist" |
                            "build" |
                            "output" |
                            "vendor" |
                            "venv"
                    )
                {
                    continue;
                }
                let kind = entry.file_type().await?;
                ensure!(
                    !kind.is_symlink(),
                    "quick build rejects symlinks: {}",
                    path.display()
                );
                if kind.is_dir() {
                    pending.push(path);
                } else if supported(&path) ||
                    super::super::collect::LocalImageFile::supported(&path)
                {
                    ensure!(
                        files.len() < 256,
                        "quick build exceeds 256 sources; select a smaller \
                         directory or configure a publication"
                    );
                    files.push(path);
                }
            }
        }
        files.sort();
        ensure!(
            !files.is_empty(),
            "no supported Markdown or RDF sources found in {}",
            source.display()
        );
        Ok(Self { root: source.to_owned(), files, definitions: None })
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
