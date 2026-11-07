use {
    anyhow::{Result, ensure},
    std::{collections::BTreeSet, path::Path},
};

pub(super) async fn appended_source(
    root: &Path,
    roots: &[String],
    source: &str,
) -> Result<String> {
    ensure!(
        !roots.is_empty(),
        "auto_append requires at least one snippet base_path"
    );
    let mut matches = BTreeSet::new(); // bounded: at most eight explicitly configured roots.
    for base in roots {
        let path = root.join(base).join(source);
        match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file(),
                    "auto_append source must be a regular file: {}",
                    path.display()
                );
                let path = tokio::fs::canonicalize(&path).await?;
                ensure!(
                    path.starts_with(root),
                    "auto_append source escapes project root"
                );
                matches.insert(path);
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error.into()),
        }
    }
    ensure!(
        !matches.is_empty(),
        "auto_append source {source} was not found under configured snippet \
         roots"
    );
    ensure!(
        matches.len() == 1,
        "auto_append source {source} is ambiguous across snippet roots"
    );
    super::super::page::source_path(root, &matches.into_iter().next().unwrap())
}
