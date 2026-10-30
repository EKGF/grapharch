//! Configurable content and metadata gates for publication profiles.
use {
    crate::publication::{
        diagnostics::{Diagnostic, Severity, SourceContext},
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeSet,
};

pub(super) struct Requirements {
    // bounded: profiles permit at most 64 required section roles.
    missing: BTreeSet<String>,
}

impl Requirements {
    pub async fn read(
        view: &RevisionView,
        profile: &NamedNode,
        target: &NamedNode,
    ) -> Result<Self> {
        let missing =
            Self::strings(view, profile, "requiredSectionRole", 64).await?;
        let mut rows = view.objects(profile, "requiredMetadata").await?;
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= 64,
                "profile exceeds 64 metadata requirements"
            );
            let Term::NamedNode(predicate) = &row["value"] else {
                anyhow::bail!("requiredMetadata must identify an RDF predicate")
            };
            let mut values = view.objects_by(target, predicate).await?;
            let mut supplied = 0;
            while let Some(row) = values.next().await? {
                supplied += 1;
                ensure!(
                    supplied <= 64,
                    "required metadata {predicate} exceeds value budget"
                );
                if let Term::Literal(value) = &row["value"] {
                    ensure!(
                        !value.value().trim().is_empty(),
                        "required metadata {predicate} is empty on {target}"
                    );
                }
            }
            ensure!(
                supplied > 0,
                "missing required metadata {predicate} on {target}"
            );
        }
        let statuses =
            Self::strings(view, profile, "allowedPublicationStatus", 16)
                .await?;
        if !statuses.is_empty() {
            let status = view.text(target, "publicationStatus").await?;
            ensure!(
                statuses.contains(&status),
                "publication status {status:?} is not allowed by profile \
                 {profile}"
            );
        }
        Ok(Self { missing })
    }

    async fn strings(
        view: &RevisionView,
        subject: &NamedNode,
        property: &str,
        limit: usize,
    ) -> Result<BTreeSet<String>> {
        let mut rows = view.objects(subject, property).await?;
        let mut values = BTreeSet::new(); // bounded: the explicit profile-field limit.
        while let Some(row) = rows.next().await? {
            let Term::Literal(value) = &row["value"] else {
                anyhow::bail!("{property} must be a literal")
            };
            ensure!(
                value.value() == value.value().trim() &&
                    !value.value().is_empty() &&
                    value.value().len() <= 128,
                "{property} must be nonempty, trimmed and at most 128 bytes"
            );
            values.insert(value.value().to_owned());
            ensure!(
                values.len() <= limit,
                "{property} exceeds profile budget"
            );
        }
        Ok(values)
    }

    pub async fn role(
        &self,
        view: &RevisionView,
        placement: &NamedNode,
    ) -> Result<Option<String>> {
        if self.missing.is_empty() {
            return Ok(None);
        }
        match view.optional(placement, "sectionRole").await? {
            None => Ok(None),
            Some(Term::Literal(role)) => {
                Ok(self
                    .missing
                    .contains(role.value())
                    .then(|| role.value().to_owned()))
            },
            _ => anyhow::bail!("sectionRole must be a literal on {placement}"),
        }
    }

    pub async fn selected(
        &mut self,
        view: &RevisionView,
        role: Option<&str>,
        fragment: &NamedNode,
    ) -> Result<()> {
        let Some(role) = role else { return Ok(()) };
        if self.missing.contains(role) &&
            !view.text(fragment, "body").await?.trim().is_empty()
        {
            self.missing.remove(role);
        }
        Ok(())
    }

    pub fn finish(&self) -> Result<()> {
        ensure!(
            self.missing.is_empty(),
            "publication is missing required selected section roles: {}",
            self.missing.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        Ok(())
    }
}

pub(super) async fn failure(
    writer: &GraphWriter,
    project: &NamedNode,
    target: &NamedNode,
    revision: &NamedNode,
    error: anyhow::Error,
) -> Result<anyhow::Error> {
    writer
        .record_diagnostic_at(
            SourceContext {
                project:        project.as_str(),
                processor:      "urn:grapharch:standards-profile-validator",
                source:         target.as_str(),
                stage:          "plan",
                input_revision: Some(revision),
            },
            Diagnostic {
                severity:        Severity::Error,
                code:            "publication-requirement-failed".into(),
                message:         format!("{error:#}"),
                reference:       target.as_str().into(),
                source_fragment: None,
            },
        )
        .await?;
    Ok(error)
}
