//! One bounded authored abbreviation dictionary becomes source-owned SKOS
//! facts.
use {
    super::{Collector, SourceIdentity, source::FileInput},
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        writer::ContributionStream,
    },
    anyhow::{Context, Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::{
        collections::BTreeMap,
        path::{Component, Path},
    },
    tokio::{fs::File, io::AsyncReadExt},
};
const MAX_BYTES: usize = 128 * 1024;
const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
pub(super) struct AbbreviationFile {
    input:    FileInput,
    relative: String,
    scheme:   NamedNode,
}
impl AbbreviationFile {
    pub async fn configured(
        project: &str,
        processor: &NamedNode,
        source: &str,
        root: &Path,
    ) -> Result<Self> {
        ensure!(
            !source.is_empty() &&
                Path::new(source)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "abbreviation source must be project-relative without traversal: \
             {source}"
        );
        let path = tokio::fs::canonicalize(root.join(source))
            .await
            .with_context(|| format!("abbreviation source {source}"))?;
        ensure!(
            path.starts_with(root) &&
                tokio::fs::metadata(&path).await?.is_file(),
            "abbreviation source escapes project root or is not a file: \
             {source}"
        );
        Ok(Self {
            input:    FileInput::new(project, processor.as_str(), path)?,
            relative: source.replace('\\', "/"),
            scheme:   identity("abbreviation-scheme", &[
                project,
                processor.as_str(),
            ]),
        })
    }

    async fn produce_source(
        &self,
        stream: &mut ContributionStream,
    ) -> Result<String> {
        let mut bytes = Vec::new(); // bounded: one dictionary, at most 128 KiB + one overflow byte.
        File::open(&self.input.path)
            .await?
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .await?;
        ensure!(
            bytes.len() <= MAX_BYTES,
            "abbreviation dictionary exceeds 128 KiB"
        );
        let source = String::from_utf8(bytes)
            .context("abbreviation dictionary must be UTF-8")?;
        let dictionary = Dictionary::parse(&source)?;
        let mut digest = Sha256::new();
        digest.update(self.relative.as_bytes());
        digest.update([0]);
        digest.update(source.as_bytes());
        let revision = format!(
            "abbreviation-file-v1:{}",
            hex::encode(digest.finalize())
        );
        let fragment = self.input.fragment_id();
        for quad in self.input.description(
            &source,
            "text/plain",
            Some(self.scheme.clone()),
            Some("Abbreviation dictionary source"),
            &revision,
        )? {
            stream.write(quad).await?;
        }
        for quad in [
            self.input.quad(
                &fragment,
                "sourcePath",
                Literal::new_simple_literal(&self.relative),
            ),
            self.input.quad(
                &self.input.project,
                "abbreviationScheme",
                self.scheme.clone(),
            ),
            standard(
                &self.scheme,
                RDF_TYPE,
                NamedNode::new_unchecked(format!("{SKOS}ConceptScheme")),
            ),
            standard(
                &self.scheme,
                &format!("{SKOS}prefLabel"),
                Literal::new_simple_literal("Abbreviations"),
            ),
            self.input
                .quad(&self.scheme, "derivedFrom", fragment.clone()),
        ] {
            stream.write(quad).await?;
        }
        for (notation, definition) in dictionary.0 {
            let concept =
                identity("abbreviation", &[self.scheme.as_str(), &notation]);
            for quad in [
                standard(
                    &concept,
                    RDF_TYPE,
                    NamedNode::new_unchecked(format!("{SKOS}Concept")),
                ),
                standard(
                    &concept,
                    &format!("{SKOS}notation"),
                    Literal::new_simple_literal(&notation),
                ),
                standard(
                    &concept,
                    &format!("{SKOS}prefLabel"),
                    Literal::new_simple_literal(notation),
                ),
                standard(
                    &concept,
                    &format!("{SKOS}definition"),
                    Literal::new_simple_literal(definition.meaning),
                ),
                standard(
                    &concept,
                    &format!("{SKOS}inScheme"),
                    self.scheme.clone(),
                ),
                self.input.quad(&concept, "derivedFrom", fragment.clone()),
            ] {
                stream.write(quad).await?;
            }
        }
        Ok(revision)
    }
}
#[async_trait]
impl Collector for AbbreviationFile {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.input.project.as_str(),
            processor: self.input.collector.as_str(),
            source:    self.input.source()?,
        })
    }

    async fn produce(&self, stream: &mut ContributionStream) -> Result<String> {
        self.produce_source(stream)
            .await
            .with_context(|| format!("abbreviation source {}", self.relative))
    }
}
fn standard(
    subject: &NamedNode,
    predicate: &str,
    value: impl Into<Term>,
) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked(predicate),
        value,
        GraphName::DefaultGraph,
    )
}
struct Definition {
    meaning: String,
    line:    usize,
}
struct Dictionary(BTreeMap<String, Definition>); // bounded: 1024 entries and 128 KiB authored text.
impl Dictionary {
    fn parse(source: &str) -> Result<Self> {
        ensure!(
            !source.contains('\0'),
            "abbreviation dictionary contains a NUL byte"
        );
        let mut entries: BTreeMap<String, Definition> = BTreeMap::new();
        for (index, line) in source.lines().enumerate() {
            let number = index + 1;
            let line = line.trim();
            if line.is_empty() ||
                line.strip_prefix("[//]:").is_some_and(|comment| {
                    comment.trim_start().starts_with('#')
                })
            {
                continue;
            }
            let (notation, meaning) = line
                .strip_prefix("*[")
                .and_then(|line| line.split_once("]:"))
                .with_context(|| {
                    format!(
                        "malformed abbreviation at line {number}; expected \
                         *[TERM]: Definition"
                    )
                })?;
            ensure!(
                !notation.is_empty() &&
                    notation.len() <= 128 &&
                    notation.trim() == notation &&
                    !notation.chars().any(|ch| {
                        ch.is_control() || matches!(ch, '[' | ']')
                    }),
                "abbreviation at line {number} requires 1–128 bytes without \
                 brackets or control characters"
            );
            let meaning = meaning.trim();
            ensure!(
                !meaning.is_empty() &&
                    meaning.len() <= 4096 &&
                    !meaning.chars().any(char::is_control),
                "abbreviation definition at line {number} requires 1–4096 \
                 bytes of text"
            );
            if let Some(previous) = entries.get(notation) {
                anyhow::bail!(
                    "duplicate abbreviation {notation} at line {number}; \
                     first declared at line {}",
                    previous.line
                );
            }
            ensure!(
                entries.len() < 1024,
                "abbreviation dictionary exceeds 1024 entries at line {number}"
            );
            entries.insert(notation.to_owned(), Definition {
                meaning: meaning.to_owned(),
                line:    number,
            });
        }
        ensure!(
            !entries.is_empty(),
            "abbreviation dictionary contains no definitions"
        );
        Ok(Self(entries))
    }
}
