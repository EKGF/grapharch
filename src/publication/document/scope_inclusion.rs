//! Scope membership of configured collector uses, bound from the subjects of
//! the fragments they actually contributed. Collectors never choose scopes.
use {
    crate::publication::{
        dataset::{Dataset, Stage},
        term,
        view::RevisionView,
        writer::{ContributionStream, GraphWriter},
    },
    anyhow::{Context, Result, bail, ensure},
    oxrdf::{GraphName, NamedNode, Quad, Term},
};

/// Engine processor owning collector-use scope bindings.
pub const PROCESSOR: &str = "urn:grapharch:scope-inclusion";
const SCOPES_PER_USE: usize = 16;
const SUBJECTS_PER_FRAGMENT: usize = 64;

/// Write `scope ga:include subject` for every collector use with
/// `ga:includedIn`, reading the uses from a definitions-only view and the
/// fragment subjects from the collected KG.
pub async fn bind(
    dataset: &Dataset,
    definitions: &RevisionView,
    project: &NamedNode,
    writer: &GraphWriter,
) -> Result<()> {
    let collected = dataset.snapshot_for(project.as_str()).await?;
    let view = RevisionView::new(dataset.clone(), collected.clone());
    let mut stream = writer
        .begin_stage(
            project.as_str(),
            PROCESSOR,
            project.as_str(),
            &format!("scope-inclusion-v1:{collected}"),
            Stage::Document,
        )
        .await?;
    match write(definitions, &view, project, &mut stream).await {
        Ok(()) => stream.complete().await,
        Err(error) => {
            stream.abort().await?;
            Err(error)
        },
    }
}

async fn write(
    definitions: &RevisionView,
    view: &RevisionView,
    project: &NamedNode,
    stream: &mut ContributionStream,
) -> Result<()> {
    let mut uses = Vec::new(); // bounded: the configured collector budget.
    let mut rows = definitions.objects(project, "collector").await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            uses.len() <
                crate::publication::collect::configured_git::COLLECTOR_USES,
            "{project} exceeds the configured collector budget"
        );
        let Term::NamedNode(processor) = &row["value"] else {
            bail!("{project} ga:collector must identify an IRI")
        };
        uses.push(processor.clone());
    }
    uses.sort();
    for processor in &uses {
        let scopes = scopes(definitions, processor).await?;
        if scopes.is_empty() {
            continue;
        }
        let fragment = definitions
            .node(processor, "fragment")
            .await
            .with_context(|| {
                format!("{processor} ga:includedIn requires its ga:fragment")
            })?;
        let mut subjects = view.objects(&fragment, "subject").await?;
        let mut count = 0;
        while let Some(row) = subjects.next().await? {
            count += 1;
            ensure!(
                count <= SUBJECTS_PER_FRAGMENT,
                "{fragment} exceeds the {SUBJECTS_PER_FRAGMENT}-subject budget"
            );
            let Term::NamedNode(subject) = &row["value"] else {
                bail!("{fragment} ga:subject must be an IRI")
            };
            for scope in &scopes {
                stream
                    .write(Quad::new(
                        scope.clone(),
                        term("include"),
                        subject.clone(),
                        GraphName::DefaultGraph,
                    ))
                    .await?;
            }
        }
        ensure!(
            count > 0,
            "{processor} ga:includedIn: no collected fragment {fragment} with \
             a ga:subject"
        );
    }
    Ok(())
}

/// The scopes one configured use is included in, validated against the
/// definitions.
pub(crate) async fn scopes(
    definitions: &RevisionView,
    processor: &NamedNode,
) -> Result<Vec<NamedNode>> {
    let mut scopes = Vec::new(); // bounded: SCOPES_PER_USE.
    let mut rows = definitions.objects(processor, "includedIn").await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            scopes.len() < SCOPES_PER_USE,
            "{processor} exceeds the {SCOPES_PER_USE}-scope inclusion budget"
        );
        let Term::NamedNode(scope) = &row["value"] else {
            bail!("{processor} ga:includedIn must identify a ga:Scope")
        };
        ensure!(
            definitions.has_type(scope, "Scope").await?,
            "{processor} ga:includedIn references undefined ga:Scope {scope}"
        );
        scopes.push(scope.clone());
    }
    scopes.sort();
    Ok(scopes)
}

#[cfg(test)]
mod tests;
