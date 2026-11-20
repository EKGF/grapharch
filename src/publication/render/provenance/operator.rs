//! Recorded build runs that used a collected source commit.
use {
    super::{
        model::BuildOperator,
        read::{CONTROL, PROV, SCHEMA},
    },
    crate::publication::render::model::{Plan, node},
    anyhow::{Result, ensure},
    oxrdf::Term,
};

const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";

pub(super) async fn read(plan: &Plan) -> Result<Vec<BuildOperator>> {
    let pattern = plan.source.pattern(
        &format!(
            "?repo a <{SCHEMA}SoftwareSourceCode> ; <{PROV}hadMember> ?commit \
             . ?run a <{PROV}Activity> ; <{PROV}used> ?commit ; \
             <{RDFS_LABEL}> ?label"
        ),
        0,
    );
    let mut runs = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?run ?label WHERE {{ {pattern} {CONTROL} }} \
             ORDER BY ?run ?label"
        ))
        .await?;
    let mut operators = Vec::new(); // bounded: sixteen recorded build runs.
    while let Some(row) = runs.next().await? {
        ensure!(
            operators.len() < 16,
            "provenance build operator limit exceeded"
        );
        let run = node(&row["run"])?;
        let Term::Literal(label) = &row["label"] else {
            anyhow::bail!("build run label must be a literal")
        };
        let link = matches!(
            url::Url::parse(run.as_str())?.scheme(),
            "https" | "http"
        )
        .then(|| run.as_str().to_owned());
        operators.push(BuildOperator {
            label:  label.value().into(),
            run:    link,
            agents: agents(plan, run).await?,
        });
    }
    Ok(operators)
}

async fn agents(plan: &Plan, run: &oxrdf::NamedNode) -> Result<Vec<String>> {
    let pattern = plan.source.pattern(
        &format!(
            "{run} <{PROV}wasAssociatedWith> ?agent . ?agent <{RDFS_LABEL}> \
             ?name"
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?name WHERE {{ {pattern} {CONTROL} }} ORDER BY \
             ?name"
        ))
        .await?;
    let mut agents = Vec::new(); // bounded: eight agents per build run.
    while let Some(row) = rows.next().await? {
        ensure!(agents.len() < 8, "build run agent limit exceeded");
        let Term::Literal(name) = &row["name"] else {
            anyhow::bail!("build run agent label must be a literal")
        };
        agents.push(name.value().into());
    }
    Ok(agents)
}
