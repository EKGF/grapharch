use {
    super::super::{content::Entry, model::Plan},
    anyhow::{Result, ensure},
    oxrdf::Term,
};

pub(super) async fn aliases(entry: &Entry, plan: &Plan) -> Result<Vec<String>> {
    let mut aliases = Vec::new();
    for field in ["nativeAnchor", "headingAnchor"] {
        if let Some(value) =
            plan.source.optional(&entry.fragment, field).await?
        {
            let Term::Literal(value) = value else {
                anyhow::bail!("{field} requires a literal")
            };
            ensure!(
                !value.value().is_empty() && value.value().len() <= 512,
                "invalid native anchor"
            );
            if !aliases.iter().any(|a| a == value.value()) {
                aliases.push(value.value().to_owned());
            }
        }
    }
    Ok(aliases)
}
