use {
    anyhow::{Result, ensure},
    std::{collections::BTreeSet, path::Path},
};

pub(super) struct Inventory {
    pub paths: BTreeSet<String>,
    bytes:     u64,
    entries:   usize,
}
impl Inventory {
    pub fn new() -> Self {
        Self { paths: BTreeSet::new(), bytes: 0, entries: 0 }
    }

    pub async fn copy(
        &mut self,
        source: &Path,
        root: &Path,
        prefix: &str,
    ) -> Result<()> {
        ensure!(
            tokio::fs::metadata(source).await?.is_dir(),
            "artifact input must be a directory"
        );
        // bounded: traversal has an 8192-entry ceiling across all source trees.
        let mut pending = vec![(source.to_path_buf(), prefix.to_owned())];
        while let Some((directory, target)) = pending.pop() {
            let mut children = tokio::fs::read_dir(&directory).await?;
            while let Some(child) = children.next_entry().await? {
                self.entries += 1;
                ensure!(
                    self.entries <= 8192,
                    "hosting artifact exceeds entry budget"
                );
                let name = child.file_name().into_string().map_err(|_| {
                    anyhow::anyhow!("artifact paths must be UTF-8")
                })?;
                ensure!(
                    name != ".git" &&
                        name != ".env" &&
                        !name.starts_with(".env.") &&
                        name != ".vercel",
                    "private source file is not a publication artifact: {name}"
                );
                super::relative(&name)?;
                let path = if target.is_empty() {
                    name
                } else {
                    format!("{target}/{name}")
                };
                let kind = child.file_type().await?;
                ensure!(
                    !kind.is_symlink(),
                    "artifact symlinks are not supported"
                );
                if kind.is_dir() {
                    pending.push((child.path(), path));
                    continue;
                }
                ensure!(kind.is_file(), "artifact must be a regular file");
                ensure!(
                    self.paths.len() < 4096 && self.paths.insert(path.clone()),
                    "artifact file budget or path collision: {path}"
                );
                self.bytes = self
                    .bytes
                    .checked_add(child.metadata().await?.len())
                    .ok_or_else(|| {
                        anyhow::anyhow!("artifact byte count overflow")
                    })?;
                ensure!(
                    self.bytes <= 512 * 1024 * 1024,
                    "hosting artifact exceeds 512 MiB"
                );
                let destination = root.join(&path);
                tokio::fs::create_dir_all(
                    destination.parent().expect("destination has parent"),
                )
                .await?;
                tokio::fs::copy(child.path(), destination).await?;
            }
        }
        Ok(())
    }
}
