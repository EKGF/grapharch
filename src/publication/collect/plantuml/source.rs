//! Retain a bounded, local-only PlantUML include closure before documenting it.
use {
    anyhow::{Result, ensure},
    std::{
        collections::BTreeMap,
        future::Future,
        path::{Path, PathBuf},
        pin::Pin,
    },
    tokio::io::AsyncReadExt,
};
pub(super) struct Expanded {
    pub code:    String,
    pub sources: BTreeMap<PathBuf, String>,
}
pub(super) async fn expand(root: &Path, entry: &Path) -> Result<Expanded> {
    let root = tokio::fs::canonicalize(root).await?;
    let mut result =
        Expanded { code: String::new(), sources: BTreeMap::new() };
    visit(&root, entry, &mut Vec::new(), &mut result).await?;
    Ok(result)
}
fn visit<'a>(
    root: &'a Path,
    entry: &'a Path,
    active: &'a mut Vec<PathBuf>,
    result: &'a mut Expanded,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        ensure!(
            active.len() < 8,
            "PlantUML include depth exceeds 8 files"
        );
        let path = tokio::fs::canonicalize(entry).await?;
        ensure!(
            path.starts_with(root),
            "PlantUML include escapes project root"
        );
        ensure!(!active.contains(&path), "PlantUML include cycle");
        if result.sources.contains_key(&path) {
            return Ok(());
        }
        ensure!(
            result.sources.len() < 32,
            "PlantUML include closure exceeds 32 files"
        );
        let file = tokio::fs::File::open(&path).await?;
        ensure!(
            file.metadata().await?.len() <= 64 * 1024,
            "PlantUML source exceeds 64 KiB"
        );
        let mut body = String::new(); // bounded: one capped source file plus overflow byte.
        file.take(64 * 1024 + 1).read_to_string(&mut body).await?;
        ensure!(
            body.len() <= 64 * 1024,
            "PlantUML source exceeds 64 KiB"
        );
        let retained: usize = result.sources.values().map(String::len).sum();
        ensure!(
            retained + body.len() <= 128 * 1024,
            "PlantUML retained include closure exceeds 128 KiB"
        );
        result.sources.insert(path.clone(), body.clone());
        active.push(path.clone());
        for line in body.lines() {
            let trimmed = line.trim();
            if let Some(include) = trimmed
                .strip_prefix("!include ")
                .or_else(|| trimmed.strip_prefix("!include_once "))
            {
                let include = include.trim().trim_matches('"');
                ensure!(
                    !include.is_empty() &&
                        !include.starts_with(['/', '<']) &&
                        !include.contains([':', '\\', '%', '$']),
                    "PlantUML includes require explicit local paths"
                );
                let include = path
                    .parent()
                    .expect("canonical file has parent")
                    .join(include);
                visit(root, &include, active, result).await?;
            } else {
                ensure!(
                    !trimmed.starts_with("!include") &&
                        !trimmed.starts_with("!import") &&
                        !trimmed.starts_with("!theme"),
                    "PlantUML remote, dynamic and implicit theme includes are \
                     unsupported; use a local include"
                );
                result.code.push_str(line);
                result.code.push('\n');
                ensure!(
                    result.code.len() <= 128 * 1024,
                    "PlantUML expanded diagram exceeds 128 KiB"
                );
            }
        }
        active.pop();
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn local_include_expands_but_network_cycles_and_escape_fail()
    -> Result<()> {
        let root = tempfile::tempdir()?;
        tokio::fs::create_dir(root.path().join("theme")).await?;
        tokio::fs::write(
            root.path().join("theme/light.puml"),
            "skinparam backgroundColor transparent\n",
        )
        .await?;
        let entry = root.path().join("diagram.puml");
        tokio::fs::write(
            &entry,
            "@startuml\n!include theme/light.puml\nclass A\n@enduml\n",
        )
        .await?;
        let output = expand(root.path(), &entry).await?;
        assert!(
            output
                .code
                .contains("skinparam backgroundColor transparent")
        );
        for invalid in [
            "!include https://example.org/a.puml",
            "!include ../outside.puml",
            "!include diagram.puml",
        ] {
            tokio::fs::write(&entry, invalid).await?;
            assert!(
                expand(root.path(), &entry).await.is_err(),
                "{invalid}"
            );
        }
        Ok(())
    }
}
