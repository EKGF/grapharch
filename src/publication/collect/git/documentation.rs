//! Explicit documentation transport reuses the locked bare cache and commit
//! pin.
use super::*;

/// Stable source identity shared by remote configuration and Git blob
/// collectors.
pub fn documentation_source(
    repository: &str,
    relative: &Path,
) -> Result<String> {
    validate_path(relative)?;
    let origin = public_repository_url(repository)?.to_string();
    Ok(identity("git-file", &[
        &origin,
        relative.to_str().context("Git path must be UTF-8")?,
    ])
    .into_string())
}

/// Pin a documentation repository without pretending it is an RDF repository.
/// Selection and materialization remain explicit and never execute source code.
pub async fn remote_documentation(
    url: &str,
    reference: &str,
    cache: &Path,
    project: &str,
) -> Result<RepositoryScan> {
    ensure!(
        !reference.is_empty(),
        "a Git ref or commit is required"
    );
    let (url, reference, cache, project) = (
        url.to_owned(),
        reference.to_owned(),
        cache.to_owned(),
        project.to_owned(),
    );
    tokio::task::spawn_blocking(move || {
        Ok(scan_remote_selected(
            &url,
            &reference,
            &cache,
            Some(&project),
            None,
            false,
        )?
        .0)
    })
    .await?
}

fn validate_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty() &&
            path.components()
                .all(|part| matches!(part, std::path::Component::Normal(_))),
        "documentation selection must be repository-relative without traversal"
    );
    Ok(())
}
