//! Static HTML and embedded Typst/PDF publication from immutable target plans.

mod callouts;
mod content;
mod diagnostic_graph;
mod engine;
mod files;
mod html;
mod markdown_html;
mod markup;
mod model;
mod native_typst;
mod record;
mod specification;
mod typst;
mod typst_markdown;
mod website;
pub use model::PublicationManifest;
pub(crate) use model::title as page_title;
use {
    crate::publication::{
        dataset::Dataset,
        diagnostics::FailureMode,
        plan::ResolvedPlan,
    },
    anyhow::{Result, ensure},
    files::{Staging, Ticket},
    model::Plan,
    std::{
        future::Future,
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    },
    tokio::sync::{Mutex, OnceCell, Semaphore},
    uuid::Uuid,
};

/// A successfully promoted artifact generation.
pub struct Publication {
    /// Target-owned artifact directory.
    pub directory: PathBuf,
    /// Provenance identifying the input, plan, and output owner.
    pub manifest:  PublicationManifest,
}

/// Superseded work never replaces a newer accepted request's artifacts.
pub enum PublishOutcome {
    /// A complete generation was promoted.
    Published(Box<Publication>),
    /// A newer request superseded this work.
    Superseded,
}

/// Reusable publication runtime. Clones share bounded workers and compiler
/// assets.
#[derive(Clone)]
pub struct Publisher {
    failure_mode:  FailureMode,
    dataset:       Dataset,
    session:       String,
    sequence:      Arc<AtomicU64>,
    coordination:  Arc<Mutex<()>>,
    workers:       Arc<Semaphore>,
    compile_slots: Arc<Semaphore>,
    assets:        Arc<OnceCell<Arc<engine::Assets>>>,
}

impl Publisher {
    /// Create a runtime for one shared dataset. Reuse it for editor previews.
    pub fn new(dataset: Dataset) -> Self {
        Self {
            failure_mode: FailureMode::FailFast,
            dataset,
            session: Uuid::new_v4().to_string(),
            sequence: Arc::new(AtomicU64::new(1)),
            coordination: Arc::new(Mutex::new(())),
            workers: Arc::new(Semaphore::new(2)),
            compile_slots: Arc::new(Semaphore::new(1)),
            assets: Arc::new(OnceCell::new()),
        }
    }

    /// Choose explicit diagnostic-preview behavior. Default is fail-fast.
    pub fn with_failure_mode(mut self, mode: FailureMode) -> Self {
        self.failure_mode = mode;
        self
    }

    /// Schedule a render. Request order is assigned synchronously, before the
    /// returned future is polled, so reordered task scheduling cannot revive
    /// old work.
    pub fn render(
        &self,
        plan: &ResolvedPlan,
        root: &Path,
    ) -> impl Future<Output = Result<PublishOutcome>> + Send + 'static {
        let sequence = self.next_sequence();
        let publisher = self.clone();
        let plan = plan.clone();
        let root = root.to_owned();
        let job = Uuid::new_v4().to_string();
        async move {
            let sequence = sequence?;
            publisher.run(plan, root, sequence, job).await
        }
    }

    fn next_sequence(&self) -> Result<u64> {
        let mut current = self.sequence.load(Ordering::Relaxed);
        loop {
            let next = current.checked_add(1).ok_or_else(|| {
                anyhow::anyhow!("publication request sequence overflow")
            })?;
            match self.sequence.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(current),
                Err(observed) => current = observed,
            }
        }
    }

    async fn run(
        &self,
        handle: ResolvedPlan,
        root: PathBuf,
        sequence: u64,
        job: String,
    ) -> Result<PublishOutcome> {
        let _worker = self.workers.clone().acquire_owned().await?;
        let plan = Plan::read(self.dataset.clone(), handle).await?;
        ensure!(
            crate::publication::Renderer::parse(&plan.renderer).is_ok(),
            "unsupported publication engine"
        );
        let destination = files::prepare(&root, &plan).await?;
        let ticket = Ticket {
            session: self.session.clone(),
            sequence,
            job: job.clone(),
            project: plan.project.as_str().to_owned(),
            target: plan.target.as_str().to_owned(),
        };
        record::begin(&plan, &destination, &job, self.failure_mode).await?;
        let result = self.publish(&plan, destination, ticket, &job).await;
        if let Err(error) = record::finish(&plan, &job, &result).await {
            let context = match &result {
                Ok(PublishOutcome::Published(_)) => {
                    "artifacts were promoted, but publication recording failed"
                        .to_owned()
                },
                Ok(PublishOutcome::Superseded) => {
                    "superseded publication recording failed".to_owned()
                },
                Err(render_error) => {
                    format!(
                        "render failed ({render_error}); publication \
                         recording also failed"
                    )
                },
            };
            return Err(error.context(context));
        }
        result
    }

    async fn publish(
        &self,
        plan: &Plan,
        destination: PathBuf,
        ticket: Ticket,
        job: &str,
    ) -> Result<PublishOutcome> {
        if !files::register(
            destination.clone(),
            ticket.clone(),
            self.coordination.clone(),
        )
        .await?
        {
            return Ok(PublishOutcome::Superseded);
        }
        let stage = Staging::new(&destination).await?;
        let result = async {
            let website_files = if plan.renderer == "website" {
                Some(website::write(plan, &stage.path).await?)
            } else {
                None
            };
            let diagnostics = if website_files.is_some() {
                Vec::new()
            } else {
                content::write(plan, &stage.path, self.failure_mode).await?
            };
            if plan.is_html() {
                let style = if plan.renderer == "specification" {
                    specification::STYLE
                } else {
                    html::STYLE
                };
                tokio::fs::write(
                    stage.path.join("style.css"),
                    format!("{style}\n{}", include_str!("callouts.css")),
                )
                .await?;
            }
            if matches!(plan.renderer.as_str(), "typst" | "specification") {
                ensure!(
                    tokio::fs::metadata(stage.path.join("main.typ"))
                        .await?
                        .len() <=
                        64 * 1024 * 1024,
                    "Typst source exceeds compilation budget"
                );
                let assets = self
                    .assets
                    .get_or_try_init(engine::Assets::load)
                    .await?
                    .clone();
                let pdf = engine::compile(
                    stage.path.clone(),
                    assets,
                    self.compile_slots.clone(),
                )
                .await?;
                ensure!(
                    pdf.len() <= 128 * 1024 * 1024,
                    "PDF exceeds artifact byte budget"
                );
                tokio::fs::write(stage.path.join("document.pdf"), pdf).await?;
            }
            let mut manifest = plan.manifest(job);
            if let Some(files) = website_files {
                manifest.files = files;
            }
            manifest.diagnostics = diagnostics;
            manifest.failure_mode = self.failure_mode;
            tokio::fs::write(
                stage.path.join("manifest.yaml"),
                serde_yaml::to_string(&manifest)?,
            )
            .await?;
            if !files::promote(
                stage.path.clone(),
                destination.clone(),
                ticket,
                self.coordination.clone(),
            )
            .await?
            {
                return Ok(PublishOutcome::Superseded);
            }
            Ok(PublishOutcome::Published(Box::new(Publication {
                directory: destination,
                manifest,
            })))
        }
        .await;
        stage.cleanup().await?;
        result
    }
}
