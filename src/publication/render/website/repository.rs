//! Repository identity comes from the pinned project, never the template.
use {
    super::super::{markup::html_escape as esc, model::Plan},
    anyhow::{Result, ensure},
    oxrdf::Term,
};

pub(super) async fn link(plan: &Plan) -> Result<String> {
    let Some(repository) =
        plan.source.optional(&plan.project, "repository").await?
    else {
        return Ok(String::new());
    };
    let Term::NamedNode(repository) = repository else {
        anyhow::bail!("project repository must be an IRI")
    };
    let url = url::Url::parse(repository.as_str())?;
    ensure!(
        url.scheme() == "https" &&
            url.host_str().is_some() &&
            url.username().is_empty() &&
            url.password().is_none(),
        "repository must be an HTTPS web URL without credentials"
    );
    let (label, content) = if matches!(
        url.host_str(),
        Some("github.com" | "www.github.com")
    ) {
        (
            "View source on GitHub",
            include_str!("icons/github.svg"),
        )
    } else {
        ("View source repository", "Source")
    };
    Ok(format!(
        "<a class=\"source-link\" href=\"{}\" aria-label=\"{label}\" \
         title=\"{label}\">{content}</a>",
        esc(repository.as_str())
    ))
}
