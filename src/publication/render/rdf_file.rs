//! An RDF renderer target owns one file per declared RDF format inside a
//! destination directory that several file-owning targets may share. Its
//! manifest records each file's media type and any negotiated path, for
//! hosting platforms that route requests.
use {
    super::{
        Publication,
        PublishOutcome,
        Publisher,
        files,
        model::PublicationManifest,
        owned_file::{COORDINATION, OwnedFile},
        rdf_plan::RdfPlan,
        record,
    },
    crate::publication::rdf_target::{
        NegotiatedPath,
        PublishedRdfFile,
        RdfTarget,
    },
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
    std::{
        fs,
        path::{Path, PathBuf},
    },
    uuid::Uuid,
};

/// What an RDF target's manifest adds for hosting: paths are relative to the
/// output root, as declared destinations are.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RdfPublication {
    pub destination: String,
    pub files:       Vec<PublishedRdfFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negotiation: Option<NegotiationRecord>,
}

/// A negotiated path and the entry document it serves by default.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NegotiationRecord {
    pub path: NegotiatedPath,
    pub html: String,
}

impl RdfPublication {
    /// The directory of an output root holding each RDF target's current
    /// manifest, one `<key>.yaml` per target.
    pub(crate) fn index(root: &Path) -> PathBuf {
        root.join(COORDINATION).join("targets")
    }
}

impl Publisher {
    pub(super) async fn run_rdf(
        &self,
        handle: crate::publication::plan::ResolvedPlan,
        root: PathBuf,
        job: String,
    ) -> Result<PublishOutcome> {
        let plan = RdfPlan::read(self.dataset.clone(), handle).await?;
        let destination =
            files::prepare(&root, &plan.destination, &plan.targets).await?;
        record::begin(
            &plan.subject,
            &destination,
            &job,
            self.failure_mode,
        )
        .await?;
        let result = self.publish_rdf(&plan, &root, destination, &job).await;
        record::conclude(&plan.subject, &job, result).await
    }

    async fn publish_rdf(
        &self,
        plan: &RdfPlan,
        root: &Path,
        destination: PathBuf,
        job: &str,
    ) -> Result<PublishOutcome> {
        let subject = &plan.subject;
        ensure!(
            !tokio::fs::try_exists(destination.join("manifest.yaml")).await?,
            "RDF target {} cannot publish into {}: that directory is owned by \
             a directory publication",
            subject.target,
            destination.display()
        );
        let target =
            RdfTarget::read(&plan.source, &subject.project, &subject.target)
                .await?;
        let root = tokio::fs::canonicalize(root).await?;
        let coordination = root.join(COORDINATION);
        tokio::fs::create_dir_all(&coordination).await?;
        let stage = coordination.join(format!("stage-{}", Uuid::new_v4()));
        tokio::fs::create_dir(&stage).await?;
        let result = async {
            let published = target.write(&plan.source, &stage).await?;
            let manifest = PublicationManifest {
                version:        1,
                project:        subject.project.as_str().to_owned(),
                target:         subject.target.as_str().to_owned(),
                input_revision: subject.revision.as_str().to_owned(),
                plan:           subject.handle.id.as_str().to_owned(),
                plan_snapshot:  subject.handle.snapshot.as_str().to_owned(),
                scope:          None,
                profile:        None,
                renderer:       subject.renderer.as_str().to_owned(),
                job:            job.to_owned(),
                files:          published
                    .iter()
                    .map(|file| file.name.as_str().to_owned())
                    .collect(),
                books:          Vec::new(),
                failure_mode:   self.failure_mode,
                diagnostics:    Vec::new(),
                pdf:            None,
                rdf:            Some(RdfPublication {
                    destination: plan.destination.clone(),
                    files:       published,
                    negotiation: target.negotiation.clone().map(
                        |negotiation| {
                            NegotiationRecord {
                                path: negotiation.path,
                                html: negotiation.html,
                            }
                        },
                    ),
                }),
            };
            let yaml = serde_yaml::to_string(&manifest)?;
            let (stage, root, to) =
                (stage.clone(), root.clone(), destination.clone());
            let manifest = tokio::task::spawn_blocking(move || {
                Promotion { coordination, stage, root, destination: to }
                    .run(&manifest, &yaml)
                    .map(|()| manifest)
            })
            .await??;
            Ok(PublishOutcome::Published(Box::new(Publication {
                source: destination.clone(),
                directory: destination.clone(),
                manifest,
            })))
        }
        .await;
        match tokio::fs::remove_dir_all(&stage).await {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error.into());
            },
            _ => {},
        }
        result
    }
}

/// Replace an RDF target's files together: every lease and ownership check
/// precedes the first replacement, and files the target no longer declares
/// are withdrawn.
struct Promotion {
    coordination: PathBuf,
    stage:        PathBuf,
    root:         PathBuf,
    destination:  PathBuf,
}

impl Promotion {
    fn run(&self, manifest: &PublicationManifest, yaml: &str) -> Result<()> {
        let index = RdfPublication::index(&self.root).join(format!(
            "{}.yaml",
            OwnedFile::key(Path::new(&format!(
                "{}\n{}",
                manifest.project, manifest.target
            )))
        ));
        let previous = match fs::read(&index) {
            Ok(bytes) => {
                serde_yaml::from_slice::<PublicationManifest>(&bytes)?.files
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Vec::new()
            },
            Err(error) => return Err(error.into()),
        };
        let owned = |name: &str| {
            OwnedFile::new(
                &self.coordination,
                &self.destination,
                name,
                &self.root,
            )
        };
        // bounded: an RDF target declares at most eight formats.
        let mut current = Vec::with_capacity(manifest.files.len());
        for name in &manifest.files {
            let file = owned(name)?;
            let lease = file.lease()?;
            file.check_owner(
                &lease,
                &self.destination,
                &manifest.project,
                &manifest.target,
            )?;
            current.push((name, file, lease));
        }
        let mut withdrawn = Vec::new();
        for name in previous
            .iter()
            .filter(|name| !manifest.files.contains(name))
        {
            let file = owned(name)?;
            let lease = file.lease()?;
            file.check_owner(
                &lease,
                &self.destination,
                &manifest.project,
                &manifest.target,
            )?;
            withdrawn.push((file, lease));
        }
        for (name, file, lease) in &current {
            let record = self.stage.join(format!("record-{name}"));
            fs::create_dir(&record)?;
            fs::write(record.join("manifest.yaml"), yaml)?;
            file.replace(
                lease,
                &self.stage.join(name),
                &record,
                &self.destination,
                &[],
            )?;
        }
        for (file, lease) in &withdrawn {
            file.withdraw(lease, &self.destination)?;
        }
        fs::create_dir_all(index.parent().expect("target index parent"))?;
        let next = index.with_extension(format!("next-{}", Uuid::new_v4()));
        fs::write(&next, yaml)?;
        fs::rename(next, index)?;
        Ok(())
    }
}
