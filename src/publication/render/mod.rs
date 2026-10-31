//! Static HTML and embedded Typst/PDF publication from immutable target plans.

mod book_worker;
mod bookshelf;
mod callouts;
mod collected_images;
mod content;
mod diagnostic_graph;
mod engine;
mod files;
mod html;
mod markdown_html;
mod markup;
mod model;
mod native_typst;
mod preamble;
mod quality;
mod record;
mod specification;
mod typst;
mod typst_markdown;
mod typst_style;
mod website;
pub(crate) async fn compile_book(root: &Path, main: &str) -> Result<()> {
    book_worker::compile(root, main).await
}
pub use model::{BookPublication, PublicationManifest};
pub(crate) fn book_compiler_fingerprint() -> String {
    bookshelf::compiler_digest()
}
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
    book_executable: Option<PathBuf>,
    book_jobs:       usize,
    failure_mode:    FailureMode,
    dataset:         Dataset,
    session:         String,
    sequence:        Arc<AtomicU64>,
    coordination:    Arc<Mutex<()>>,
    workers:         Arc<Semaphore>,
    compile_slots:   Arc<Semaphore>,
    assets:          Arc<OnceCell<Arc<engine::Assets>>>,
}

impl Publisher {
    /// Create a runtime for one shared dataset. Reuse it for editor previews.
    pub fn new(dataset: Dataset) -> Self {
        Self {
            book_executable: None,
            book_jobs: 2,
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

    /// Choose the native CLI executable and bounded PDF worker count.
    pub fn with_book_workers(
        mut self,
        executable: PathBuf,
        jobs: usize,
    ) -> Result<Self> {
        ensure!(
            executable.is_file(),
            "book worker executable does not exist"
        );
        ensure!(
            (1..=8).contains(&jobs),
            "book workers must be between 1 and 8"
        );
        self.book_executable = Some(executable);
        self.book_jobs = jobs;
        Ok(self)
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
        let mut plan = Plan::read(self.dataset.clone(), handle).await?;
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
        let result = async {
            if plan.renderer != "website" {
                plan.images = collected_images::Images::read(&plan).await?;
            }
            self.publish(&plan, destination, ticket, &job).await
        }
        .await;
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
            let image_files = if plan.renderer == "website" {
                Vec::new()
            } else {
                plan.images.write(plan, &stage.path).await?
            };
            let website_files = if plan.renderer == "website" {
                Some(website::write(plan, &stage.path).await?)
            } else {
                None
            };
            let page_files = if plan.renderer == "specification-pages" {
                Some(
                    specification::pages::write(
                        plan,
                        &stage.path,
                        self.failure_mode,
                    )
                    .await?,
                )
            } else {
                None
            };
            let book_files = if plan.renderer == "bookshelf" {
                Some(
                    Box::pin(bookshelf::write(
                        plan,
                        &stage.path,
                        &destination,
                        self.failure_mode,
                        self.book_executable.as_deref(),
                        self.book_jobs,
                    ))
                    .await?,
                )
            } else {
                None
            };
            let diagnostics = if let Some((_, diagnostics)) = &book_files {
                diagnostics.clone()
            } else if let Some((_, diagnostics)) = &page_files {
                diagnostics.clone()
            } else if website_files.is_some() {
                plan.quality.clone()
            } else {
                content::write(plan, &stage.path, self.failure_mode).await?
            };
            if plan.is_html() {
                let style = if matches!(
                    plan.renderer.as_str(),
                    "specification" | "specification-pages" | "bookshelf"
                ) {
                    specification::STYLE
                } else {
                    html::STYLE
                };
                tokio::fs::write(
                    stage.path.join("style.css"),
                    format!(
                        "{style}\n{}\n{}",
                        include_str!("callouts.css"),
                        if plan.renderer == "bookshelf" {
                            include_str!("bookshelf.css")
                        } else {
                            ""
                        }
                    ),
                )
                .await?;
            }
            if matches!(plan.renderer.as_str(), "typst" | "specification") {
                ensure!(
                    tokio::fs::metadata(stage.path.join("main.typ"))
                        .await?
                        .len() <=
                        crate::publication::limits::TYPST_SOURCE_BYTES,
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
                    pdf.len() as u64 <=
                        crate::publication::limits::ARTIFACT_BYTES,
                    "PDF exceeds artifact byte budget"
                );
                tokio::fs::write(stage.path.join("document.pdf"), pdf).await?;
            }
            let mut manifest = plan.manifest(job);
            if let Some(files) = website_files {
                manifest.files = files;
            }
            if let Some((files, _)) = page_files {
                manifest.files = files;
            }
            if let Some((files, _)) = book_files {
                manifest.files = files;
                manifest.books =
                    bookshelf::manifest_books(plan, &stage.path).await?;
            }
            manifest.files.extend(image_files);
            manifest.diagnostics = diagnostics;
            manifest.failure_mode = self.failure_mode;
            let metadata = serde_yaml::to_string(&manifest)?;
            ensure!(
                metadata.len() as u64 <= files::METADATA_LIMIT,
                "publication manifest exceeds byte budget"
            );
            tokio::fs::write(stage.path.join("manifest.yaml"), metadata)
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

#[cfg(test)]
mod future_budget_tests {
    use super::*;
    #[test]
    fn publication_future_stays_within_the_stack_budget() -> Result<()> {
        let publisher = Publisher::new(Dataset::memory()?);
        let plan = ResolvedPlan {
            id:       oxrdf::NamedNode::new("urn:test:plan")?,
            snapshot: oxrdf::NamedNode::new("urn:test:snapshot")?,
        };
        let future = publisher.render(&plan, Path::new("unused"));
        let bytes = std::mem::size_of_val(&future);
        ensure!(
            bytes < 64 * 1024,
            "publication future uses {bytes} stack bytes"
        );
        Ok(())
    }
}
