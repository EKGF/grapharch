use {
    anyhow::{Result, ensure},
    std::path::{Component, Path, PathBuf},
};
pub(super) async fn discover(
    root: &Path,
    source: &str,
) -> Result<Vec<(String, PathBuf)>> {
    ensure!(
        !source.is_empty() &&
            Path::new(source)
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "use-case source must be project-relative without traversal"
    );
    let directory = tokio::fs::canonicalize(root.join(source)).await?;
    ensure!(
        directory.starts_with(root),
        "use-case source escapes project root"
    );
    let mut pending = vec![directory.clone()]; // bounded: at most 256 source directories.
    let mut result = Vec::new(); // bounded: at most 256 authored use-case pages.
    let mut count = 0;
    while let Some(path) = pending.pop() {
        let mut entries = tokio::fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            count += 1;
            ensure!(
                count <= 1024,
                "authored use-case subtree exceeds 1024 entries"
            );
            let name = entry.file_name();
            if name.to_string_lossy().starts_with(['.', '_']) {
                continue;
            }
            let kind = entry.file_type().await?;
            ensure!(
                !kind.is_symlink(),
                "use-case subtree cannot contain symbolic links"
            );
            if kind.is_dir() {
                ensure!(
                    pending.len() < 256,
                    "use-case directory budget exceeded"
                );
                pending.push(entry.path());
            } else if entry.path().extension().is_some_and(|ext| ext == "md") {
                let path = entry.path();
                let relative = path
                    .strip_prefix(&directory)?
                    .to_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("use-case path must be UTF-8")
                    })?
                    .replace('\\', "/");
                if relative == "index.md" {
                    continue;
                }
                let id = relative
                    .strip_suffix("/index.md")
                    .or_else(|| relative.strip_suffix(".md"))
                    .unwrap()
                    .to_owned();
                ensure!(
                    result.len() < 256,
                    "use-case source exceeds 256 pages"
                );
                result.push((id, path));
            }
        }
    }
    result.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(result)
}
pub(super) fn resolve(id: &str, reference: &str) -> Result<String> {
    ensure!(
        !reference.is_empty() &&
            reference.len() <= 2048 &&
            !reference.contains('\\'),
        "invalid authored use-case reference"
    );
    let mut parts: Vec<&str> = if reference.starts_with('/') {
        Vec::new()
    } else {
        id.split('/').collect()
    }; // bounded: source/reference paths at most 2048 bytes.
    for part in reference.trim_start_matches('/').split('/') {
        match part {
            ".." => {
                ensure!(
                    parts.pop().is_some(),
                    "use-case reference escapes subtree"
                );
            },
            "." => {},
            "" => anyhow::bail!("empty use-case reference segment"),
            part => {
                ensure!(
                    part.chars().all(|ch| {
                        ch.is_ascii_alphanumeric() ||
                            matches!(ch, '-' | '_' | '.')
                    }),
                    "invalid use-case path segment"
                );
                parts.push(part);
            },
        }
    }
    ensure!(
        !parts.is_empty(),
        "use-case reference selects the collection overview, not a UseCase; \
         independent roots require explicit is-part-of: []"
    );
    Ok(parts.join("/"))
}
