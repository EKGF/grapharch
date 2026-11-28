//! The `build_plantuml` plugin's directory layout, read as data and wired to
//! one `ga:PlantUmlFileCollector` use per diagram source. Diagrams render
//! locally with the pinned PlantUML runtime; no server is contacted.
use {
    anyhow::{Result, ensure},
    serde_yaml::Value,
    std::path::{Component, Path, PathBuf},
};

/// Project-relative diagram directories declared by `build_plantuml`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PlantUmlHint {
    root:   PathBuf,
    input:  PathBuf,
    output: PathBuf,
}
impl PlantUmlHint {
    pub(super) fn read(options: Option<&Value>) -> Result<Self> {
        let text = |key: &str, default: &str| -> Result<PathBuf> {
            let value = options
                .and_then(|o| o.get(key))
                .map(|v| {
                    v.as_str().ok_or_else(|| {
                        anyhow::anyhow!("build_plantuml.{key} must be text")
                    })
                })
                .transpose()?
                .unwrap_or(default);
            let path = PathBuf::from(value);
            ensure!(
                !value.is_empty() &&
                    path.components()
                        .all(|c| matches!(c, Component::Normal(_))),
                "build_plantuml.{key} must be relative without traversal"
            );
            Ok(path)
        };
        Ok(Self {
            root:   text("diagram_root", "docs/diagrams")?,
            input:  text("input_folder", "src")?,
            output: text("output_folder", "out")?,
        })
    }

    /// The project-relative source directory.
    pub(super) fn sources(&self) -> PathBuf { self.root.join(&self.input) }

    /// The project-relative SVG that `build_plantuml` writes for `source`.
    pub(super) fn destination(&self, source: &Path) -> Option<PathBuf> {
        let relative = source.strip_prefix(self.sources()).ok()?;
        Some(
            self.root
                .join(&self.output)
                .join(relative)
                .with_extension("svg"),
        )
    }
}

impl PlantUmlHint {
    /// The project-relative `.puml` sources, sorted, at most `limit`.
    pub(super) async fn files(
        &self,
        project: &Path,
        limit: usize,
    ) -> Result<Vec<PathBuf>> {
        // bounded: at most `limit` diagram sources.
        let mut found = Vec::new();
        let mut pending = vec![(self.sources(), 0_usize)];
        while let Some((directory, depth)) = pending.pop() {
            let mut entries =
                match tokio::fs::read_dir(project.join(&directory)).await {
                    Ok(entries) => entries,
                    // A configured but absent directory has no diagrams.
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        continue
                    },
                    Err(error) => return Err(error.into()),
                };
            while let Some(entry) = entries.next_entry().await? {
                let kind = entry.file_type().await?;
                let path = directory.join(entry.file_name());
                if kind.is_dir() && depth < 4 {
                    pending.push((path, depth + 1));
                } else if kind.is_file() &&
                    path.extension().is_some_and(|e| e == "puml")
                {
                    found.push(path);
                    ensure!(
                        found.len() <= limit,
                        "build_plantuml sources exceed {limit} diagrams"
                    );
                }
            }
        }
        found.sort();
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_mirrors_the_plugin_layout() {
        let hint = PlantUmlHint::read(Some(
            &serde_yaml::from_str(
                "diagram_root: docs/diagrams\ninput_folder: \
                 src\noutput_folder: out\n",
            )
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(
            hint.destination(Path::new("docs/diagrams/src/a/b.puml")),
            Some(PathBuf::from("docs/diagrams/out/a/b.svg"))
        );
        assert!(
            PlantUmlHint::read(Some(
                &serde_yaml::from_str("diagram_root: ../x\n").unwrap()
            ))
            .is_err()
        );
    }
}
