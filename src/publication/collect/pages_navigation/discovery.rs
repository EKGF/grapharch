//! Asset directories do not create empty documentation navigation sections.
use {
    crate::publication::website::exclusions::{EntryKind, SourceExclusions},
    anyhow::{Result, ensure},
    std::path::Path,
};
pub(super) async fn has_markdown(
    directory: &Path,
    exclusions: &SourceExclusions,
) -> Result<bool> {
    let mut pending = vec![directory.to_owned()]; // bounded: 512 discovered directories.
    let mut visited = 0;
    let mut directories = 1;
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            visited += 1;
            ensure!(
                visited <= 4096,
                "navigation subtree discovery exceeds 4096 entries"
            );
            let kind = entry.file_type().await?;
            if exclusions.excludes(&entry.path(), EntryKind::of(kind.is_dir())) ||
                entry.file_name().to_string_lossy().starts_with(['.', '_'])
            {
                continue;
            }
            ensure!(!kind.is_symlink(), "navigation rejects symlinks");
            if kind.is_file() &&
                entry.path().extension().is_some_and(|ext| ext == "md")
            {
                return Ok(true);
            }
            if kind.is_dir() {
                directories += 1;
                ensure!(
                    directories <= 512,
                    "navigation subtree exceeds 512 directories"
                );
                pending.push(entry.path());
            }
        }
    }
    Ok(false)
}
