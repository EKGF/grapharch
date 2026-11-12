//! The run configuration of one showcase: RDF properties of exactly one
//! `ga:Showcase` in a bounded Turtle file beside the showcase's inputs. Each
//! runner names the GraphArch predicates it reads; any other GraphArch
//! predicate on the showcase is an error, so a misspelt setting never falls
//! back to a default.
use {
    super::{NAMESPACE, RDF_TYPE, term},
    anyhow::{Context, Result, bail, ensure},
    oxigraph::{io::RdfFormat, store::Store},
    oxrdf::{
        GraphNameRef,
        NamedNode,
        NamedNodeRef,
        NamedOrBlankNode,
        NamedOrBlankNodeRef,
        Term,
    },
    std::path::{Component, Path, PathBuf},
    tokio::io::AsyncReadExt,
};

const CONFIGURATION_BYTES: u64 = 64 * 1024;
const LIST_ITEMS: usize = 64;
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// GraphArch predicates recording the provenance of copied fixture files.
/// Every runner accepts them beside its own settings.
pub(crate) const COPIED_SOURCES: &[&str] = &[
    "copiedSource",
    "copiedRepository",
    "copiedRevision",
    "copiedFromWorkingTree",
];

/// One parsed showcase configuration file.
pub(crate) struct ShowcaseConfiguration {
    id:        NamedNode,
    store:     Store,
    path:      PathBuf,
    directory: PathBuf,
}

impl ShowcaseConfiguration {
    /// Parse `path`, which must describe exactly one `ga:Showcase`.
    pub(crate) async fn read(path: &Path) -> Result<Self> {
        let path = tokio::fs::canonicalize(path).await.with_context(|| {
            format!("showcase configuration {}", path.display())
        })?;
        let directory = path
            .parent()
            .context("showcase configuration requires a parent directory")?
            .to_owned();
        let mut bytes = Vec::new(); // bounded: CONFIGURATION_BYTES + 1.
        tokio::fs::File::open(&path)
            .await?
            .take(CONFIGURATION_BYTES + 1)
            .read_to_end(&mut bytes)
            .await?;
        ensure!(
            bytes.len() as u64 <= CONFIGURATION_BYTES,
            "showcase configuration {} exceeds 64 KiB",
            path.display()
        );
        let (id, store) = tokio::task::spawn_blocking(move || -> Result<_> {
            let store = Store::new()?;
            store.load_from_reader(RdfFormat::Turtle, bytes.as_slice())?;
            let mut showcases = store.quads_for_pattern(
                None,
                Some(NamedNodeRef::new_unchecked(RDF_TYPE)),
                Some(term("Showcase").as_ref().into()),
                Some(GraphNameRef::DefaultGraph),
            );
            let Some(first) = showcases.next().transpose()? else {
                bail!("showcase configuration declares no ga:Showcase")
            };
            ensure!(
                showcases.next().transpose()?.is_none(),
                "showcase configuration declares more than one ga:Showcase"
            );
            let NamedOrBlankNode::NamedNode(id) = first.subject else {
                bail!("a configured ga:Showcase requires a stable IRI")
            };
            Ok((id, store))
        })
        .await??;
        Ok(Self { id, store, path, directory })
    }

    /// The configured showcase.
    pub(crate) fn id(&self) -> &NamedNode { &self.id }

    /// The configuration file.
    pub(crate) fn path(&self) -> &Path { &self.path }

    /// Reject GraphArch predicates on the showcase that `runner` does not read.
    pub(crate) fn accept_only(
        &self,
        runner: &str,
        settings: &[&str],
    ) -> Result<()> {
        for quad in self.store.quads_for_pattern(
            Some(self.id.as_ref().into()),
            None,
            None,
            None,
        ) {
            let predicate = quad?.predicate;
            let Some(name) = predicate.as_str().strip_prefix(NAMESPACE) else {
                continue;
            };
            ensure!(
                settings.contains(&name) || COPIED_SOURCES.contains(&name),
                "{runner} showcase {} does not support ga:{name}",
                self.id
            );
        }
        Ok(())
    }

    fn values(
        &self,
        subject: NamedOrBlankNodeRef<'_>,
        property: &str,
    ) -> Result<Vec<Term>> {
        let mut values = Vec::new(); // bounded: LIST_ITEMS.
        for quad in self.store.quads_for_pattern(
            Some(subject),
            Some(term(property).as_ref()),
            None,
            None,
        ) {
            ensure!(
                values.len() < LIST_ITEMS,
                "{subject} has more than {LIST_ITEMS} ga:{property} values"
            );
            values.push(quad?.object);
        }
        values.sort_by_key(ToString::to_string);
        Ok(values)
    }

    fn optional_of(
        &self,
        subject: NamedOrBlankNodeRef<'_>,
        property: &str,
    ) -> Result<Option<Term>> {
        let mut values = self.values(subject, property)?;
        ensure!(
            values.len() <= 1,
            "{subject} has conflicting ga:{property} values"
        );
        Ok(values.pop())
    }

    /// An optional literal setting.
    pub(crate) fn optional_text(
        &self,
        property: &str,
    ) -> Result<Option<String>> {
        Self::literal(
            self.optional_of(self.id.as_ref().into(), property)?,
            &self.id,
            property,
        )
    }

    /// A required literal setting.
    pub(crate) fn text(&self, property: &str) -> Result<String> {
        self.optional_text(property)?.with_context(|| {
            format!("showcase {} requires ga:{property}", self.id)
        })
    }

    /// Every literal value of a repeatable setting, in lexical order.
    pub(crate) fn texts(&self, property: &str) -> Result<Vec<String>> {
        self.values(self.id.as_ref().into(), property)?
            .into_iter()
            .map(|value| {
                Self::literal(Some(value), &self.id, property)
                    .map(|text| text.expect("present value"))
            })
            .collect()
    }

    /// An optional IRI setting.
    pub(crate) fn optional_node(
        &self,
        property: &str,
    ) -> Result<Option<NamedNode>> {
        match self.optional_of(self.id.as_ref().into(), property)? {
            None => Ok(None),
            Some(Term::NamedNode(node)) => Ok(Some(node)),
            Some(_) => {
                bail!(
                    "showcase {} ga:{property} must be an IRI",
                    self.id
                )
            },
        }
    }

    /// A required IRI setting.
    pub(crate) fn node(&self, property: &str) -> Result<NamedNode> {
        self.optional_node(property)?.with_context(|| {
            format!("showcase {} requires ga:{property}", self.id)
        })
    }

    /// Every IRI value of a repeatable setting, in IRI order.
    pub(crate) fn nodes(&self, property: &str) -> Result<Vec<NamedNode>> {
        self.values(self.id.as_ref().into(), property)?
            .into_iter()
            .map(|value| {
                match value {
                    Term::NamedNode(node) => Ok(node),
                    _ => {
                        bail!(
                            "showcase {} ga:{property} must be an IRI",
                            self.id
                        )
                    },
                }
            })
            .collect()
    }

    /// A required literal of another configured resource, such as a document.
    pub(crate) fn text_of(
        &self,
        subject: &NamedNode,
        property: &str,
    ) -> Result<String> {
        Self::literal(
            self.optional_of(subject.as_ref().into(), property)?,
            subject,
            property,
        )?
        .with_context(|| format!("{subject} requires ga:{property}"))
    }

    /// An optional `xsd:boolean` setting.
    pub(crate) fn flag(&self, property: &str) -> Result<Option<bool>> {
        match self.optional_of(self.id.as_ref().into(), property)? {
            None => Ok(None),
            Some(Term::Literal(value))
                if value.datatype().as_str() ==
                    "http://www.w3.org/2001/XMLSchema#boolean" =>
            {
                Ok(Some(matches!(value.value(), "true" | "1")))
            },
            Some(_) => {
                bail!(
                    "showcase {} ga:{property} must be an xsd:boolean",
                    self.id
                )
            },
        }
    }

    /// The IRIs of an ordered `rdf:List` setting, if present.
    pub(crate) fn node_list(
        &self,
        property: &str,
    ) -> Result<Option<Vec<NamedNode>>> {
        let Some(mut cell) =
            self.optional_of(self.id.as_ref().into(), property)?
        else {
            return Ok(None);
        };
        let mut items = Vec::new(); // bounded: LIST_ITEMS.
        loop {
            let subject = match &cell {
                Term::NamedNode(node) if node.as_str() == RDF_NIL => break,
                Term::BlankNode(node) => NamedOrBlankNodeRef::from(node),
                _ => {
                    bail!(
                        "showcase {} ga:{property} must be an RDF list",
                        self.id
                    )
                },
            };
            ensure!(
                items.len() < LIST_ITEMS,
                "showcase {} ga:{property} exceeds {LIST_ITEMS} items",
                self.id
            );
            let first = self.single(subject, RDF_FIRST)?;
            let Term::NamedNode(item) = first else {
                bail!(
                    "showcase {} ga:{property} items must be IRIs",
                    self.id
                )
            };
            items.push(item);
            cell = self.single(subject, RDF_REST)?;
        }
        Ok(Some(items))
    }

    fn single(
        &self,
        subject: NamedOrBlankNodeRef<'_>,
        predicate: &str,
    ) -> Result<Term> {
        let mut quads = self.store.quads_for_pattern(
            Some(subject),
            Some(NamedNodeRef::new_unchecked(predicate)),
            None,
            None,
        );
        let value = quads
            .next()
            .transpose()?
            .with_context(|| format!("malformed RDF list at {subject}"))?;
        ensure!(
            quads.next().transpose()?.is_none(),
            "malformed RDF list at {subject}"
        );
        Ok(value.object)
    }

    /// An exact lowercase 40-character Git commit setting.
    pub(crate) fn commit(&self, property: &str, label: &str) -> Result<String> {
        let commit = self.text(property)?;
        super::revision_build::exact_commit(&commit, label)?;
        Ok(commit)
    }

    /// A required relative path beneath the configuration directory.
    pub(crate) async fn local_file(&self, property: &str) -> Result<PathBuf> {
        let relative = self.text(property)?;
        let path = Path::new(&relative);
        ensure!(
            !relative.is_empty() &&
                path.components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "showcase {} ga:{property} must be a relative path beneath its \
             configuration",
            self.id
        );
        let resolved =
            tokio::fs::canonicalize(self.directory.join(path)).await?;
        ensure!(
            resolved.starts_with(&self.directory),
            "showcase {} ga:{property} escapes its configuration directory",
            self.id
        );
        Ok(resolved)
    }

    /// The recorded provenance of every copied fixture file.
    pub(crate) fn copied_sources(&self) -> Result<Vec<CopiedSource>> {
        let mut sources = Vec::new(); // bounded: LIST_ITEMS values.
        for value in self.values(self.id.as_ref().into(), "copiedSource")? {
            let subject = match &value {
                Term::NamedNode(node) => NamedOrBlankNodeRef::from(node),
                Term::BlankNode(node) => NamedOrBlankNodeRef::from(node),
                Term::Literal(_) => {
                    bail!(
                        "showcase {} ga:copiedSource must be a resource",
                        self.id
                    )
                },
            };
            let text = |property| {
                Self::literal(
                    self.optional_of(subject, property)?,
                    &subject,
                    property,
                )
            };
            let local_path = text("localPath")?
                .with_context(|| format!("{subject} requires ga:localPath"))?;
            ensure!(
                Path::new(&local_path)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
                "{subject} ga:localPath must be relative beneath the showcase"
            );
            let digest = text("contentDigest")?.with_context(|| {
                format!("{subject} requires ga:contentDigest")
            })?;
            ensure!(
                digest.len() == 64 &&
                    digest.bytes().all(|b| {
                        b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                    }),
                "{subject} ga:contentDigest must be a lowercase SHA-256 digest"
            );
            sources.push(CopiedSource {
                path: self.directory.join(local_path),
                digest,
                origin: match self.optional_of(subject, "copiedFrom")? {
                    Some(Term::NamedNode(url)) => Some(url),
                    None => None,
                    Some(_) => bail!("{subject} ga:copiedFrom must be an IRI"),
                },
            });
        }
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(sources)
    }

    fn literal(
        value: Option<Term>,
        subject: &impl std::fmt::Display,
        property: &str,
    ) -> Result<Option<String>> {
        match value {
            None => Ok(None),
            Some(Term::Literal(literal)) => {
                ensure!(
                    !literal.value().is_empty(),
                    "{subject} ga:{property} cannot be empty"
                );
                Ok(Some(literal.value().to_owned()))
            },
            Some(_) => bail!("{subject} ga:{property} must be a literal"),
        }
    }
}

/// One copied fixture file and its recorded SHA-256 digest.
pub(crate) struct CopiedSource {
    path:   PathBuf,
    digest: String,
    origin: Option<NamedNode>,
}

impl CopiedSource {
    /// The published document the file was retrieved from, if it was not
    /// copied from a repository revision.
    pub(crate) fn origin(&self) -> Option<&NamedNode> { self.origin.as_ref() }

    /// The copied file.
    pub(crate) fn path(&self) -> &Path { &self.path }

    /// Stream the file and compare it with the recorded digest.
    pub(crate) async fn verify(&self) -> Result<()> {
        use sha2::{Digest, Sha256};
        let mut file =
            tokio::fs::File::open(&self.path).await.with_context(|| {
                format!("copied source {}", self.path.display())
            })?;
        let mut hash = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024]; // bounded: one stream buffer.
        loop {
            let length = file.read(&mut buffer).await?;
            if length == 0 {
                break;
            }
            hash.update(&buffer[..length]);
        }
        let actual = hex::encode(hash.finalize());
        ensure!(
            actual == self.digest,
            "copied source {} has SHA-256 {actual}, not the recorded {}",
            self.path.display(),
            self.digest
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests;
