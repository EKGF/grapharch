use {
    super::{Commit, Provenance, Repository},
    crate::publication::{dataset::Rows, render::model::Plan},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};
const PROV: &str = "http://www.w3.org/ns/prov#";
const DCT: &str = "http://purl.org/dc/terms/";
const SCHEMA: &str = "https://schema.org/";

pub(super) async fn read(plan: &Plan) -> Result<Provenance> {
    let mut result = Provenance::default();
    let pattern = plan.source.pattern(
        &format!(
            "?repo a <{SCHEMA}SoftwareSourceCode> ; <{SCHEMA}codeRepository> \
             ?url ; <{PROV}hadMember> ?commit"
        ),
        0,
    );
    let mut rows = plan.dataset.select(format!("SELECT DISTINCT ?repo ?url ?commit WHERE {{ {pattern} GRAPH <urn:grapharch:control> {{ ?_attempt0 <https://ekgf.org/ontology/grapharch#collector> <urn:grapharch:git-provenance> }} }} ORDER BY ?url ?commit")).await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            result.repositories.len() < 128,
            "provenance repository limit exceeded"
        );
        let node = super::super::model::node(&row["commit"])?;
        let repo = super::super::model::node(&row["repo"])?;
        let url = super::super::model::node(&row["url"])?;
        ensure!(
            matches!(
                url::Url::parse(url.as_str())?.scheme(),
                "https" | "http" | "file"
            ),
            "unsupported provenance repository URL"
        );
        let commit = literal(plan, node, &format!("{DCT}identifier")).await?;
        let tags =
            literals(plan, node, &format!("{SCHEMA}version"), 64).await?;
        let mut state =
            objects(plan, repo, &format!("{PROV}atLocation")).await?;
        let working_tree = state.next().await?.is_some();
        let mut commits = Vec::new(); // bounded: five first-parent commits.
        let mut current = Some(node.clone());
        while let Some(id) = current.take() {
            if commits.len() == 5 {
                break;
            }
            let sha = literal(plan, &id, &format!("{DCT}identifier")).await?;
            let time =
                literal(plan, &id, &format!("{PROV}generatedAtTime")).await?;
            let p = plan.source.pattern(&format!("{id} <{PROV}qualifiedAttribution> ?a . ?a <{PROV}hadRole> <urn:grapharch:git-role:committer> ; <{PROV}agent> ?person . ?person <http://xmlns.com/foaf/0.1/name> ?name"), 0);
            let mut people = plan.dataset.select(format!("SELECT DISTINCT ?name WHERE {{ {p} GRAPH <urn:grapharch:control> {{ ?_attempt0 <https://ekgf.org/ontology/grapharch#collector> <urn:grapharch:git-provenance> }} }}")).await?;
            let person = people.next().await?.ok_or_else(|| {
                anyhow::anyhow!("Git commit has no recorded committer")
            })?;
            let Term::Literal(name) = &person["name"] else {
                anyhow::bail!("Git committer name must be a literal")
            };
            ensure!(
                people.next().await?.is_none(),
                "Git commit has multiple committers"
            );
            commits.push(Commit {
                id: sha,
                committer: name.value().into(),
                time,
            });
            let mut parents =
                objects(plan, &id, &format!("{PROV}wasRevisionOf")).await?;
            current = match parents.next().await? {
                Some(row) => {
                    Some(super::super::model::node(&row["value"])?.clone())
                },
                None => None,
            };
            ensure!(
                parents.next().await?.is_none(),
                "Git first-parent metadata must be singular"
            );
        }
        let p = plan.source.pattern(
            &format!(
                "?release a <http://usefulinc.com/ns/doap#Version> ; \
                 <{PROV}wasDerivedFrom> {node} ; <{DCT}title> ?title ; \
                 <{SCHEMA}url> ?url"
            ),
            0,
        );
        let mut releases = plan
            .dataset
            .select(format!(
                "SELECT DISTINCT ?title ?url WHERE {{ {p} }} ORDER BY ?title \
                 ?url"
            ))
            .await?;
        let mut release_links = Vec::new(); // bounded: sixteen explicitly associated releases per commit.
        while let Some(release) = releases.next().await? {
            ensure!(
                release_links.len() < 16,
                "provenance release limit exceeded"
            );
            let Term::Literal(title) = &release["title"] else {
                anyhow::bail!("release title must be a literal")
            };
            let url = super::super::model::node(&release["url"])?;
            ensure!(
                matches!(
                    url::Url::parse(url.as_str())?.scheme(),
                    "https" | "http"
                ),
                "unsupported release URL"
            );
            release_links.push((title.value().into(), url.as_str().into()));
        }
        result.repositories.push(Repository {
            url: url.as_str().into(),
            commit,
            working_tree,
            tags,
            releases: release_links,
            commits,
        });
    }
    let p = plan.source.pattern(&format!("{} <{PROV}wasGeneratedBy> ?activity . ?activity <{PROV}wasAssociatedWith> ?agent . ?agent <http://www.w3.org/2000/01/rdf-schema#label> ?name", plan.target), 0);
    let mut operators = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?name WHERE {{ {p} }} ORDER BY ?name"
        ))
        .await?;
    while let Some(row) = operators.next().await? {
        ensure!(
            result.operators.len() < 16,
            "provenance build operator limit exceeded"
        );
        let Term::Literal(name) = &row["name"] else {
            anyhow::bail!("build operator label must be a literal")
        };
        result.operators.push(name.value().into());
    }
    Ok(result)
}
async fn literal(
    plan: &Plan,
    id: &NamedNode,
    predicate: &str,
) -> Result<String> {
    let mut values = objects(plan, id, predicate).await?;
    let row = values.next().await?.ok_or_else(|| {
        anyhow::anyhow!("missing provenance literal {id} <{predicate}>")
    })?;
    let Term::Literal(value) = &row["value"] else {
        anyhow::bail!("provenance value must be a literal")
    };
    ensure!(
        values.next().await?.is_none(),
        "conflicting provenance values {id} <{predicate}>"
    );
    Ok(value.value().into())
}
async fn objects(plan: &Plan, id: &NamedNode, predicate: &str) -> Result<Rows> {
    let pattern = plan
        .source
        .pattern(&format!("{id} <{predicate}> ?value"), 0);
    plan.dataset.select(format!("SELECT DISTINCT ?value WHERE {{ {pattern} GRAPH <urn:grapharch:control> {{ ?_attempt0 <https://ekgf.org/ontology/grapharch#collector> <urn:grapharch:git-provenance> }} }}")).await
}
async fn literals(
    plan: &Plan,
    id: &NamedNode,
    predicate: &str,
    limit: usize,
) -> Result<Vec<String>> {
    let mut values = Vec::new(); // bounded: explicit caller-supplied metadata limit.
    let mut rows = objects(plan, id, predicate).await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            values.len() < limit,
            "provenance metadata limit exceeded"
        );
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!("provenance value must be a literal")
        };
        values.push(value.value().into());
    }
    values.sort();
    Ok(values)
}
