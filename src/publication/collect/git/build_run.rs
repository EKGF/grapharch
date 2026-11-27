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
    actor:      Option<GitHubLogin>,
}

/// The login that triggered a run, exactly as GitHub reported it.
///
/// GitHub decides which logins exist (ordinary users, `[bot]` apps and
/// Enterprise Managed Users with their `_<shortcode>` suffix), so any
/// non-empty value is recorded; the actor never fails a build.
#[derive(Clone, Debug, PartialEq, Eq)]
struct GitHubLogin(String);

impl GitHubLogin {
    /// Recognize a reported login; an absent or blank one records no actor.
    fn new(value: Option<String>) -> Option<Self> {
        value
            .map(|value| value.trim().to_owned())
            .filter(|value| {
                !value.is_empty() &&
                    value.len() <= 256 &&
                    !value.chars().any(char::is_control)
            })
            .map(Self)
    }

    /// The agent IRI beneath the GitHub server, with the login as one
    /// percent-encoded path segment.
    fn iri(&self, server: &url::Url) -> Result<NamedNode> {
        let segment = self
            .0
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                    char::from(b).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect::<String>();
        Ok(NamedNode::new(format!("{server}{segment}"))?)
    }

    fn as_str(&self) -> &str { &self.0 }
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
        let actor = GitHubLogin::new(lookup("GITHUB_ACTOR"));
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
        let mut quads = vec![
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
                &service,
                RDF_TYPE,
                iri(format!("{PROV}SoftwareAgent"))?,
            )?,
            quad(&service, RDFS_LABEL, text("GitHub Actions"))?,
        ];
        if let Some(login) = &self.actor {
            let actor = login.iri(&self.server)?;
            quads.extend([
                quad(
                    &run,
                    &format!("{PROV}wasAssociatedWith"),
                    actor.clone().into(),
                )?,
                quad(&actor, RDF_TYPE, iri(format!("{PROV}Agent"))?)?,
                quad(&actor, RDFS_LABEL, text(login.as_str()))?,
            ]);
        }
        Ok(quads)
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
    }

    fn with_actor(actor: Option<&'static str>) -> BuildRun {
        BuildRun::from_lookup(move |name| {
            match name {
                "GITHUB_ACTOR" => actor.map(Into::into),
                _ => lookup(name),
            }
        })
        .unwrap()
        .unwrap()
    }

    fn actor_iris(run: &BuildRun) -> Vec<String> {
        let commit = NamedNode::new("urn:commit").unwrap();
        run.quads(&commit)
            .unwrap()
            .into_iter()
            .filter(|quad| {
                quad.object ==
                    NamedNode::new("http://www.w3.org/ns/prov#Agent")
                        .unwrap()
                        .into()
            })
            .map(|quad| quad.subject.to_string())
            .collect()
    }

    #[test]
    fn enterprise_managed_user_login_is_recorded() {
        let run = with_actor(Some("name-surname_shortcode"));
        assert_eq!(actor_iris(&run), [
            "<https://github.com/name-surname_shortcode>"
        ]);
    }

    #[test]
    fn bot_and_unusual_logins_are_recorded_without_failing() {
        assert_eq!(
            actor_iris(&with_actor(Some("dependabot[bot]"))),
            ["<https://github.com/dependabot%5Bbot%5D>"]
        );
        assert_eq!(actor_iris(&with_actor(Some("not a login"))), [
            "<https://github.com/not%20a%20login>"
        ]);
    }

    #[test]
    fn missing_actor_records_the_run_without_an_agent() {
        for actor in [None, Some(""), Some("  ")] {
            let run = with_actor(actor);
            assert!(actor_iris(&run).is_empty());
            assert!(run.url().ends_with("/attempts/2"));
        }
    }
}
