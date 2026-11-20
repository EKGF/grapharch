use {
    super::{
        identifiers::{CommitSha, RepositoryUrl},
        model::{Commit, Provenance, Repository},
    },
    crate::publication::{dataset::Rows, render::model::Plan},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};
pub(super) const PROV: &str = "http://www.w3.org/ns/prov#";
const DCT: &str = "http://purl.org/dc/terms/";
pub(super) const SCHEMA: &str = "https://schema.org/";
pub(super) const CONTROL: &str = "GRAPH <urn:grapharch:control> { ?_attempt0 \
                                  <https://ekgf.org/ontology/grapharch#collector> \
                                  <urn:grapharch:git-provenance> }";

pub(super) async fn read(plan: &Plan) -> Result<Provenance> {
    let mut result = Provenance::default();
    let pattern = plan.source.pattern(
        &format!(
            "?repo a <{SCHEMA}SoftwareSourceCode> ; <{SCHEMA}codeRepository> \
             ?url ; <{PROV}hadMember> ?commit"
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?repo ?url ?commit WHERE {{ {pattern} {CONTROL} \
             }} ORDER BY ?url ?commit"
        ))
        .await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            result.repositories.len() < 128,
            "provenance repository limit exceeded"
        );
        let node = super::super::model::node(&row["commit"])?;
        let repo = super::super::model::node(&row["repo"])?;
        let url = RepositoryUrl::try_new(
            super::super::model::node(&row["url"])?.as_str(),
        )?;
        let commit = CommitSha::try_new(
            &literal(plan, node, &format!("{DCT}identifier")).await?,
        )?;
        let tags =
            literals(plan, repo, &format!("{SCHEMA}version"), 64).await?;
        let mut state =
            objects(plan, repo, &format!("{PROV}atLocation")).await?;
        let working_tree = state.next().await?.is_some();
        let mut commits = Vec::new(); // bounded: five first-parent commits.
        let mut current = Some(node.clone());
        while let Some(id) = current.take() {
            if commits.len() == 5 {
                break;
            }
            let sha = CommitSha::try_new(
                &literal(plan, &id, &format!("{DCT}identifier")).await?,
            )?;
            // The author wrote the change; a merge or squash committer
            // (often the forge itself) only applied it.
            let time = chrono::DateTime::parse_from_rfc3339(
                &literal(plan, &id, &format!("{DCT}created")).await?,
            )?
            .with_timezone(&chrono::Utc);
            let p = plan.source.pattern(&format!("{id} <{PROV}qualifiedAttribution> ?a . ?a <{PROV}hadRole> <urn:grapharch:git-role:author> ; <{PROV}agent> ?person . ?person <http://xmlns.com/foaf/0.1/name> ?name"), 0);
            let mut people = plan
                .dataset
                .select(format!(
                    "SELECT DISTINCT ?name WHERE {{ {p} {CONTROL} }}"
                ))
                .await?;
            let person = people.next().await?.ok_or_else(|| {
                anyhow::anyhow!("Git commit has no recorded author")
            })?;
            let Term::Literal(name) = &person["name"] else {
                anyhow::bail!("Git author name must be a literal")
            };
            ensure!(
                people.next().await?.is_none(),
                "Git commit has multiple authors"
            );
            commits.push(Commit { id: sha, author: name.value().into(), time });
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
            url,
            commit,
            working_tree,
            tags,
            releases: release_links,
            commits,
        });
    }
    result.operators = super::operator::read(plan).await?;
    Ok(result)
}
pub(super) async fn literal(
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
    plan.dataset
        .select(format!(
            "SELECT DISTINCT ?value WHERE {{ {pattern} {CONTROL} }}"
        ))
        .await
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
