use {super::super::model::Plan, anyhow::Result, oxrdf::Term};

pub(super) async fn flag(plan: &Plan, field: &str) -> Result<bool> {
    match plan.source.optional(&plan.profile, field).await? {
        None => Ok(false),
        Some(Term::Literal(value))
            if value.datatype().as_str() ==
                "http://www.w3.org/2001/XMLSchema#boolean" =>
        {
            match value.value() {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => anyhow::bail!("invalid {field} boolean"),
            }
        },
        _ => anyhow::bail!("{field} must be an xsd:boolean"),
    }
}
