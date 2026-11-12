//! Self-contained showcase outputs. Every relative reference in a showcase's
//! published HTML must name a file that the same showcase publishes, so the
//! gallery, previews and production serve the same working links. Pages are
//! tokenized in bounded chunks, without a document tree or network access.
use {
    crate::publication::{href::Href, limits},
    anyhow::{Context, Result, bail, ensure},
    html5ever::{
        TokenizerResult,
        tendril::StrTendril,
        tokenizer::{
            BufferQueue,
            StartTag,
            TagToken,
            Token,
            TokenSink,
            TokenSinkResult,
            Tokenizer,
            TokenizerOpts,
            states::RawKind,
        },
    },
    std::{
        cell::{Cell, RefCell},
        collections::BTreeSet,
        io::Read,
        path::PathBuf,
    },
};

const CHUNK_BYTES: usize = 64 * 1024;

/// The files one showcase publishes, as slash-separated paths beneath its
/// output root.
pub(super) struct PublishedOutput {
    root:  PathBuf,
    files: BTreeSet<String>,
}

impl PublishedOutput {
    pub(super) fn new(root: PathBuf, files: BTreeSet<String>) -> Self {
        Self { root, files }
    }

    /// Fail when a relative reference on a published HTML page does not name
    /// a file of this output.
    pub(super) async fn verify_links(self) -> Result<()> {
        tokio::task::spawn_blocking(move || {
            let mut broken = BrokenLinks::default();
            for page in self.files.iter().filter(|file| file.ends_with(".html"))
            {
                self.verify_page(page, &mut broken)?;
            }
            broken.ensure_none()
        })
        .await?
    }

    fn verify_page(&self, page: &str, broken: &mut BrokenLinks) -> Result<()> {
        let mut file = std::fs::File::open(self.root.join(page))
            .with_context(|| format!("cannot read published page {page}"))?;
        ensure!(
            file.metadata()?.len() <= limits::ARTIFACT_BYTES,
            "{page} exceeds the artifact byte budget"
        );
        let tokenizer =
            Tokenizer::new(References::default(), TokenizerOpts::default());
        let queue = BufferQueue::default();
        let mut buffer = vec![0; CHUNK_BYTES];
        // bounded: one chunk plus at most three bytes of a split character.
        let mut pending = Vec::with_capacity(CHUNK_BYTES + 3);
        loop {
            let read = file.read(&mut buffer)?;
            pending.extend_from_slice(&buffer[..read]);
            let valid = match std::str::from_utf8(&pending) {
                Ok(text) => text.len(),
                Err(error) if read > 0 && error.error_len().is_none() => {
                    error.valid_up_to()
                },
                Err(_) => bail!("{page} is not valid UTF-8"),
            };
            if valid > 0 {
                queue.push_back(StrTendril::from_slice(std::str::from_utf8(
                    &pending[..valid],
                )?));
                ensure!(
                    matches!(tokenizer.feed(&queue), TokenizerResult::Done),
                    "{page} could not be tokenized"
                );
                pending.drain(..valid);
            }
            if read == 0 {
                tokenizer.end();
            }
            for value in tokenizer.sink.take(page)? {
                self.check(page, &value, broken);
            }
            if read == 0 {
                return Ok(());
            }
        }
    }

    fn check(&self, page: &str, value: &str, broken: &mut BrokenLinks) {
        let problem = match Href::parse(value) {
            Err(error) => Some(error.to_string()),
            Ok(Href::Relative(relative)) => {
                match relative.published_file(page) {
                    Err(error) => Some(error.to_string()),
                    Ok(Some(file)) if self.files.contains(&file) => None,
                    Ok(Some(file)) => {
                        Some(format!(
                            "{file} is not published by this showcase"
                        ))
                    },
                    Ok(None) => Some("it leaves the showcase output".into()),
                }
            },
            Ok(Href::HostRelative) => {
                Some("it depends on the host's mount point".into())
            },
            Ok(Href::Fragment | Href::Absolute(_)) => None,
        };
        if let Some(problem) = problem {
            broken.record(page, value, &problem);
        }
    }
}

/// URL-valued attributes of start tags, drained after every chunk.
#[derive(Default)]
struct References {
    // bounded: the attributes of the tags in one chunk.
    values: RefCell<Vec<String>>,
    base:   Cell<bool>,
}

impl References {
    fn take(&self, page: &str) -> Result<Vec<String>> {
        ensure!(
            !self.base.get(),
            "{page} declares <base>, so its relative links are not portable"
        );
        Ok(std::mem::take(&mut *self.values.borrow_mut()))
    }
}

impl TokenSink for References {
    type Handle = ();

    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        let TagToken(tag) = token else {
            return TokenSinkResult::Continue;
        };
        if tag.kind != StartTag {
            return TokenSinkResult::Continue;
        }
        self.base.set(self.base.get() || &*tag.name == "base");
        self.values.borrow_mut().extend(
            tag.attrs
                .iter()
                .filter(|attribute| {
                    matches!(
                        &*attribute.name.local,
                        "href" | "src" | "poster" | "xlink:href"
                    )
                })
                .map(|attribute| attribute.value.to_string()),
        );
        // Without a tree builder, the sink selects the text states that HTML
        // assigns to these elements; script text is never read as markup.
        match &*tag.name {
            "script" => TokenSinkResult::RawData(RawKind::ScriptData),
            "style" | "xmp" | "iframe" | "noembed" | "noframes" => {
                TokenSinkResult::RawData(RawKind::Rawtext)
            },
            "title" | "textarea" => TokenSinkResult::RawData(RawKind::Rcdata),
            "plaintext" => TokenSinkResult::Plaintext,
            _ => TokenSinkResult::Continue,
        }
    }
}

#[derive(Default)]
struct BrokenLinks {
    count:   usize,
    // bounded: at most SAMPLES descriptions.
    samples: Vec<String>,
}

impl BrokenLinks {
    const SAMPLES: usize = 16;

    fn record(&mut self, page: &str, value: &str, problem: &str) {
        self.count += 1;
        if self.samples.len() < Self::SAMPLES {
            self.samples.push(format!("{page}: {value} ({problem})"));
        }
    }

    fn ensure_none(self) -> Result<()> {
        ensure!(
            self.count == 0,
            "{} relative links in published HTML do not resolve within the \
             showcase output: {}{}",
            self.count,
            self.samples.join("; "),
            if self.count > self.samples.len() {
                "; …"
            } else {
                ""
            }
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "gallery_links_tests.rs"]
mod tests;
