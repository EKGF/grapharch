//! Contained targets publish inside their container's destination
//! (`ga:containsTarget`). Promoting the container carries their outputs into
//! its new generation instead of replacing them, and a container directory
//! created only to hold contained targets may be replaced by the container.
use {
    anyhow::{Result, ensure},
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

/// Contained destinations, relative to the container's destination.
pub(super) struct Nested<'a> {
    paths: &'a [String],
}

impl<'a> Nested<'a> {
    pub(super) fn new(paths: &'a [String]) -> Self { Self { paths } }

    pub(super) fn paths(&self) -> &'a [String] { self.paths }

    /// Whether the container has no generation yet and `destination` holds
    /// only contained outputs published before it.
    pub(super) fn shell(&self, destination: &Path) -> Result<bool> {
        if self.paths.is_empty() ||
            !destination.try_exists()? ||
            destination.join("manifest.yaml").try_exists()?
        {
            return Ok(false);
        }
        for entry in fs::read_dir(destination)? {
            let name = entry?.file_name();
            let name = name.to_string_lossy();
            ensure!(
                self.paths.iter().any(|path| {
                    let first = path.split('/').next().unwrap_or_default();
                    name == first ||
                        name == format!(".grapharch-{first}.lock") ||
                        name == format!(".grapharch-{first}.request.yaml")
                }),
                "refusing to replace an unowned output directory: {} contains \
                 {name}, which no contained target owns",
                destination.display()
            );
        }
        Ok(true)
    }

    /// Move every contained output, with its publication lock records, from
    /// the `from` generation into `to`. A file the container rendered at a
    /// contained destination is a collision.
    pub(super) fn carry(&self, from: &Path, to: &Path) -> Result<()> {
        for path in self.paths {
            for (source, target) in Self::entries(from, to, path) {
                if !source.try_exists()? {
                    continue;
                }
                ensure!(
                    !target.try_exists()?,
                    "the container output collides with contained target \
                     destination {path} at {}",
                    target.display()
                );
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(source, target)?;
            }
        }
        Ok(())
    }

    fn entries(from: &Path, to: &Path, path: &str) -> Vec<(PathBuf, PathBuf)> {
        let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
        let mut entries = vec![(from.join(path), to.join(path))];
        for record in [
            format!(".grapharch-{name}.lock"),
            format!(".grapharch-{name}.request.yaml"),
        ] {
            entries.push((
                from.join(parent).join(&record),
                to.join(parent).join(record),
            ));
        }
        entries
    }
}
