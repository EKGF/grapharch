//! Compile-time generator identity, independent of package SemVer.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct CiBuild {
    pub repository:   String,
    pub workflow_ref: String,
    pub revision:     String,
    pub run_id:       u64,
    pub run_number:   u64,
    pub run_attempt:  u64,
}
impl CiBuild {
    pub fn validate(&self, revision: Option<&str>) -> Result<(), String> {
        if self.repository.len() > 201 {
            return Err("invalid generator build repository".into());
        }
        let parts: Vec<_> = self.repository.split('/').collect(); // bounded: at most 201 input bytes.
        if parts.len() != 2 ||
            parts.iter().any(|part| {
                part.is_empty() ||
                    matches!(*part, "." | "..") ||
                    !part.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || b"-_.".contains(&b)
                    })
            })
        {
            return Err("invalid generator build repository".into());
        }
        let prefix = format!("{}/.github/workflows/", self.repository);
        let valid_workflow = self
            .workflow_ref
            .strip_prefix(&prefix)
            .and_then(|value| value.split_once('@'))
            .is_some_and(|(file, reference)| {
                !file.is_empty() &&
                    file.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || b"-_.".contains(&b)
                    }) &&
                    (file.ends_with(".yml") || file.ends_with(".yaml")) &&
                    reference.starts_with("refs/")
            });
        if self.workflow_ref.len() > 1024 ||
            self.workflow_ref.chars().any(char::is_control) ||
            !valid_workflow
        {
            return Err("invalid generator build workflow".into());
        }
        if self.run_id == 0 || self.run_number == 0 || self.run_attempt == 0 {
            return Err("generator build numbers must be positive".into());
        }
        if revision != Some(self.revision.as_str()) {
            return Err("generator build context does not match source \
                        revision"
                .into());
        }
        Ok(())
    }

    pub fn url(&self) -> String {
        format!(
            "https://github.com/{}/actions/runs/{}/attempts/{}",
            self.repository, self.run_id, self.run_attempt
        )
    }
}

#[derive(Debug)]
pub(crate) struct Identity {
    pub label:    String,
    pub channel:  &'static str,
    pub revision: String,
    pub number:   String,
    pub run_url:  String,
    pub details:  String,
}
impl Identity {
    pub fn new(
        version: &str,
        revision: Option<&str>,
        dirty: bool,
        release: Option<&str>,
        ci: Option<CiBuild>,
        pinned: bool,
    ) -> Result<Self, String> {
        if revision.is_some_and(|value| {
            value.len() != 40 ||
                value.bytes().all(|b| b == b'0') ||
                !value.bytes().all(|b| {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                })
        }) {
            return Err("generator revision must be a full lowercase Git \
                        commit"
                .into());
        }
        if release.is_some_and(|release| {
            release != version || revision.is_none() || dirty
        }) {
            return Err("release identity requires the matching package \
                        version and clean exact source revision"
                .into());
        }
        let (label, channel, number, run_url, details) = if let Some(ci) = ci {
            ci.validate(revision)?;
            if dirty {
                return Err("CI generator source must be clean".into());
            }
            let number = format!("{}.{}", ci.run_number, ci.run_attempt);
            let url = ci.url();
            (
                format!("{version} (build {number})"),
                if release.is_some() { "release" } else { "ci" },
                number.clone(),
                url.clone(),
                format!(
                    "Build {number}; source commit {}; workflow {}; {url}",
                    ci.revision, ci.workflow_ref
                ),
            )
        } else if release.is_some() {
            (
                version.to_owned(),
                "release",
                String::new(),
                String::new(),
                format!(
                    "Release {version}; source commit {}",
                    revision.unwrap()
                ),
            )
        } else if let Some(revision) = revision {
            let kind = if pinned { "source" } else { "local" };
            (
                format!(
                    "{version} ({kind} {}{})",
                    &revision[..12],
                    if dirty { " dirty" } else { "" }
                ),
                if pinned { "source" } else { "local" },
                String::new(),
                String::new(),
                format!(
                    "{kind} build; source commit {revision}; tracked source {}",
                    if dirty { "modified" } else { "clean" }
                ),
            )
        } else {
            (
                format!("{version} (source archive)"),
                "source",
                String::new(),
                String::new(),
                "Source archive; no generator commit was supplied".into(),
            )
        };
        Ok(Self {
            label,
            channel,
            number,
            run_url,
            details,
            revision: revision.unwrap_or("").into(),
        })
    }
}
