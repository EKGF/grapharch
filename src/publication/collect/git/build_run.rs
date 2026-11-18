//! The CI run that operates this build, observed from its documented
//! environment.
use {
    super::identity,
    crate::publication::RDF_TYPE,
    anyhow::{Context, Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad},
};

const PROV: &str = "http://www.w3.org/ns/prov#";
const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";

/// A GitHub Actions workflow run attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BuildRun {
    server:     url::Url,
    repository: String,
    run_id:     u64,
    attempt:    u64,
    workflow:   String,
    actor:      String,
}

impl BuildRun {
    /// Read the run from this process's environment.
    pub(super) fn from_environment() -> Result<Option<Self>> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Outside GitHub Actions no run is recorded; inside, every variable
    /// must be present and valid.
    pub(super) fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>> {
        if lookup("GITHUB_ACTIONS").as_deref() != Some("true") {
            return Ok(None);
        }
        let required = |name: &str| {
            lookup(name)
                .filter(|value| !value.is_empty())
                .with_context(|| format!("GitHub Actions run lacks {name}"))
        };
        let server = url::Url::parse(&required("GITHUB_SERVER_URL")?)?;
        ensure!(
            server.scheme() == "https" &&
                server.path() == "/" &&
                server.query().is_none() &&
                server.username().is_empty(),
            "GITHUB_SERVER_URL must be an HTTPS origin"
        );
        let repository = required("GITHUB_REPOSITORY")?;
        let parts = repository.split('/').collect::<Vec<_>>(); // bounded: validated below.
        ensure!(
            repository.len() <= 201 &&
                parts.len() == 2 &&
                parts.iter().all(|part| {
                    !part.is_empty() &&
                        !matches!(*part, "." | "..") &&
                        part.bytes().all(|b| {
                            b.is_ascii_alphanumeric() || b"-_.".contains(&b)
                        })
                }),
            "invalid GITHUB_REPOSITORY"
        );
        let number = |name: &str| -> Result<u64> {
            let value = required(name)?.parse::<u64>()?;
            ensure!(value > 0, "{name} must be positive");
            Ok(value)
        };
        let workflow = required("GITHUB_WORKFLOW")?;
        ensure!(
            workflow.len() <= 256 && !workflow.chars().any(char::is_control),
            "invalid GITHUB_WORKFLOW"
        );
        let actor = required("GITHUB_ACTOR")?;
        ensure!(
            actor.len() <= 64 &&
                actor
                    .strip_suffix("[bot]")
                    .unwrap_or(&actor)
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid GITHUB_ACTOR"
        );
        Ok(Some(Self {
            server,
            repository,
            run_id: number("GITHUB_RUN_ID")?,
            attempt: number("GITHUB_RUN_ATTEMPT")?,
            workflow,
            actor,
        }))
    }

    /// The browsable run attempt, which is also the activity's identity.
    pub(super) fn url(&self) -> String {
        format!(
            "{}{}/actions/runs/{}/attempts/{}",
            self.server, self.repository, self.run_id, self.attempt
        )
    }

    /// `prov:Activity` assertions linking the run to the commit it used.
    pub(super) fn quads(&self, commit: &NamedNode) -> Result<Vec<Quad>> {
        let run = NamedNode::new(self.url())?;
        let service = identity("build-service", &[
            self.server.as_str(),
            "github-actions",
        ]);
        let actor = NamedNode::new(format!("{}{}", self.server, self.actor))?;
        let quad = |s: &NamedNode, p: &str, o: oxrdf::Term| -> Result<Quad> {
            Ok(Quad::new(
                s.clone(),
                NamedNode::new(p)?,
                o,
                GraphName::DefaultGraph,
            ))
        };
        let iri = |value: String| -> Result<oxrdf::Term> {
            Ok(NamedNode::new(value)?.into())
        };
        let text = |value: &str| Literal::new_simple_literal(value).into();
        Ok(vec![
            quad(&run, RDF_TYPE, iri(format!("{PROV}Activity"))?)?,
            quad(
                &run,
                RDFS_LABEL,
                text(&format!(
                    "{}, run {} attempt {}",
                    self.workflow, self.run_id, self.attempt
                )),
            )?,
            quad(
                &run,
                &format!("{PROV}used"),
                commit.clone().into(),
            )?,
            quad(
                &run,
                &format!("{PROV}wasAssociatedWith"),
                service.clone().into(),
            )?,
            quad(
                &run,
                &format!("{PROV}wasAssociatedWith"),
                actor.clone().into(),
            )?,
            quad(
                &service,
                RDF_TYPE,
                iri(format!("{PROV}SoftwareAgent"))?,
            )?,
            quad(&service, RDFS_LABEL, text("GitHub Actions"))?,
            quad(&actor, RDF_TYPE, iri(format!("{PROV}Agent"))?)?,
            quad(&actor, RDFS_LABEL, text(&self.actor))?,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<String> {
        Some(
            match name {
                "GITHUB_ACTIONS" => "true",
                "GITHUB_SERVER_URL" => "https://github.com",
                "GITHUB_REPOSITORY" => "EKGF/ekg-ontologies",
                "GITHUB_RUN_ID" => "19000000001",
                "GITHUB_RUN_ATTEMPT" => "2",
                "GITHUB_WORKFLOW" => "Publish",
                "GITHUB_ACTOR" => "octocat",
                _ => return None,
            }
            .into(),
        )
    }

    #[test]
    fn github_actions_environment_names_the_run_attempt() {
        let run = BuildRun::from_lookup(lookup).unwrap().unwrap();
        assert_eq!(
            run.url(),
            "https://github.com/EKGF/ekg-ontologies/actions/runs/19000000001/attempts/2"
        );
        let commit = NamedNode::new("urn:commit").unwrap();
        let quads = run.quads(&commit).unwrap();
        assert!(quads.iter().any(|quad| {
            quad.predicate.as_str() == "http://www.w3.org/ns/prov#used" &&
                quad.object == commit.clone().into()
        }));
        assert!(quads.iter().any(|quad| {
            quad.object == Literal::new_simple_literal("octocat").into()
        }));
    }

    #[test]
    fn no_run_is_recorded_outside_github_actions() {
        assert_eq!(BuildRun::from_lookup(|_| None).unwrap(), None);
    }

    #[test]
    fn incomplete_github_actions_environment_fails() {
        let missing = |name: &str| {
            (name != "GITHUB_RUN_ID").then(|| lookup(name)).flatten()
        };
        assert!(BuildRun::from_lookup(missing).is_err());
        let invalid = |name: &str| {
            match name {
                "GITHUB_ACTOR" => Some("not a login".into()),
                _ => lookup(name),
            }
        };
        assert!(BuildRun::from_lookup(invalid).is_err());
    }
}
