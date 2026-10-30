//! One declarative legal disclosure from a specification source, without
//! executing its template or publication scripts.
use {
    super::{
        Collector,
        SourceIdentity,
        git::PinnedGitBlob,
        source::{FileInput, MAX_AUTHORED_BYTES},
    },
    crate::publication::{
        dataset::identity,
        native_html,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::NamedNode,
    scraper::{ElementRef, Html, Selector},
    std::path::PathBuf,
    url::Url,
};

/// Collect a single legal preamble and bind it to explicitly chosen targets.
pub struct HtmlPreambleFile {
    input:   FileInput,
    targets: Vec<NamedNode>,
    base:    Url,
    blob:    Option<PinnedGitBlob>,
}

impl HtmlPreambleFile {
    /// Read the unique `details.preamble-section` disclosure in an HTML source.
    /// No surrounding template expressions or JavaScript are evaluated.
    pub fn new(
        project: &str,
        processor: &str,
        path: PathBuf,
        base: &str,
        targets: &[&str],
    ) -> Result<Self> {
        ensure!(
            !targets.is_empty() && targets.len() <= 16,
            "preamble requires one to sixteen targets"
        );
        let base = Url::parse(base)?;
        ensure!(
            matches!(base.scheme(), "http" | "https" | "file"),
            "preamble base must be an HTTP(S) or file source URL"
        );
        Ok(Self {
            input: FileInput::new(project, processor, path)?,
            targets: targets
                .iter()
                .map(|target| NamedNode::new(*target))
                .collect::<Result<_, _>>()?,
            base,
            blob: None,
        })
    }

    /// Use bytes from the same cached, pinned commit as the other collectors.
    pub fn from_git(
        project: &str,
        processor: &str,
        blob: &PinnedGitBlob,
        targets: &[&str],
    ) -> Result<Self> {
        let mut collector = Self::new(
            project,
            processor,
            PathBuf::from(blob.location()),
            blob.location(),
            targets,
        )?;
        collector.blob = Some(blob.clone());
        Ok(collector)
    }

    /// Stable identity of the retained legal fragment.
    pub fn fragment_id(&self) -> NamedNode {
        match &self.blob {
            Some(blob) => {
                identity("preamble", &[
                    self.input.project.as_str(),
                    self.input.collector.as_str(),
                    blob.source(),
                ])
            },
            None => self.input.fragment_id(),
        }
    }
}

#[async_trait]
impl Collector for HtmlPreambleFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.input.project.as_str(),
            processor: self.input.collector.as_str(),
            source:    match &self.blob {
                Some(blob) => blob.source(),
                None => self.input.source()?,
            },
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        let input = match &self.blob {
            Some(blob) => {
                FileInput::supplied(
                    self.input.project.as_str(),
                    self.input.collector.as_str(),
                    self.input.path.clone(),
                    blob.source().into(),
                    self.fragment_id(),
                    String::from_utf8(blob.bytes(MAX_AUTHORED_BYTES).await?)?,
                    blob.revision(),
                )?
            },
            None => self.input.clone(),
        };
        let (source, revision) = input.authored().await?;
        let base = self.base.clone();
        let (title, body) =
            tokio::task::spawn_blocking(move || -> Result<_> {
                let document = Html::parse_document(&source);
                let selector =
                    Selector::parse("details.preamble-section").unwrap();
                let mut matches = document.select(&selector);
                let preamble = matches.next().ok_or_else(|| {
                    anyhow::anyhow!("missing legal preamble disclosure")
                })?;
                ensure!(
                    matches.next().is_none(),
                    "ambiguous legal preamble disclosures"
                );
                let mut title = None;
                let mut body = String::new();
                for child in preamble.children() {
                    if let Some(element) = ElementRef::wrap(child) {
                        if element.value().name() == "summary" {
                            ensure!(
                                title.is_none(),
                                "ambiguous legal preamble title"
                            );
                            title = Some(native_html::title(element));
                        } else {
                            body.push_str(&element.html());
                        }
                    } else if let Some(text) = child.value().as_text() {
                        body.push_str(text);
                    }
                }
                let title = title.ok_or_else(|| {
                    anyhow::anyhow!("legal preamble requires summary")
                })?;
                ensure!(
                    !title.trim().is_empty() && !body.trim().is_empty(),
                    "legal preamble cannot be empty"
                );
                Ok((
                    title,
                    native_html::read(&body, Some(&base), &mut |_| Ok(None))?
                        .html,
                ))
            })
            .await??;
        let revision = format!(
            "html-preamble-v1:{revision}:{}:{:?}",
            self.base, self.targets
        );
        let fragment = self.fragment_id();
        stream
            .push(input.description(
                &body,
                "text/html",
                Some(fragment.clone()),
                Some(&title),
                &revision,
            )?)
            .await?;
        for target in &self.targets {
            stream
                .write(input.quad(target, "preamble", fragment.clone()))
                .await?;
        }
        Ok(revision)
    }
}
