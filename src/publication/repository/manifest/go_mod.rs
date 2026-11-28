//! The module path of a `go.mod`.
use {
    super::super::package::{CodePackage, Ecosystem, PackageName, web_url},
    anyhow::{Result, ensure},
};

pub(super) fn package(
    text: &str,
    manifest: &str,
) -> Result<Option<CodePackage>> {
    let Some(module) = text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("module ")
            .map(|path| path.trim().trim_matches('"'))
    }) else {
        return Ok(None);
    };
    ensure!(
        !module.is_empty(),
        "{manifest} declares an empty module path"
    );
    let mut package = CodePackage::new(
        Ecosystem::Go,
        PackageName::try_new(module)?,
        manifest.to_owned(),
    );
    package.repository =
        web_url(Some(&format!("https://{module}"))).filter(|url| {
            matches!(url.host_str(), Some("github.com" | "gitlab.com"))
        });
    Ok(Some(package))
}
