//! One explicitly annotated RDF term becomes one reference fragment. Selection
//! and publication arrangement remain outside this documentor.
use {
    super::Documentor,
    crate::publication::{
        RDF_TYPE,
        dataset::{Dataset, Stage, identity},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    sha2::{Digest, Sha256},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

/// Generate a reference fragment for one RDF resource with an explicit label
/// and Dublin Core definition in a selected language.
pub struct AnnotatedTermDocumentor {
    dataset:            Dataset,
    snapshot:           NamedNode,
    project:            NamedNode,
    subject:            NamedNode,
    language:           String,
    prefixes:           Arc<BTreeMap<String, String>>,
    documented:         Arc<HashSet<NamedNode>>,
    source_annotations: bool,
    family_reference:   bool,
}
impl AnnotatedTermDocumentor {
    pub(crate) fn processor_identity(
        project: &str,
        language: &str,
    ) -> NamedNode {
        identity("annotated-term-documentor", &[project, language])
    }

    /// Bind one term and its annotation policy to a pinned collected graph.
    pub fn new(
        dataset: Dataset,
        snapshot: NamedNode,
        project: &str,
        subject: NamedNode,
        language: &str,
        prefixes: Arc<BTreeMap<String, String>>,
        documented: Arc<HashSet<NamedNode>>,
    ) -> Result<Self> {
        Ok(Self {
            dataset,
            snapshot,
            project: NamedNode::new(project)?,
            subject,
            language: language.into(),
            prefixes,
            documented,
            source_annotations: false,
            family_reference: false,
        })
    }

    /// Publish the source's supplied English annotations without inventing
    /// missing labels or definitions. Missing annotations become warnings.
    pub fn with_source_annotations(mut self) -> Self {
        self.source_annotations = true;
        self
    }

    /// Nest a consistently labelled term under its family's type section.
    pub(crate) fn with_family_reference(mut self) -> Self {
        self.source_annotations = true;
        self.family_reference = true;
        self
    }

    async fn anonymous_facts(
        &self,
        view: &RevisionView,
        iri: &NamedNode,
    ) -> Result<Vec<(NamedNode, Term)>> {
        let pattern = view.pattern(&format!("{iri} ?predicate ?object"), 3);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?predicate ?object WHERE {{ {pattern} }} \
                 ORDER BY ?predicate ?object LIMIT 65"
            ))
            .await?;
        let mut facts = Vec::new();
        while let Some(row) = rows.next().await? {
            ensure!(
                facts.len() < 64,
                "anonymous RDF expression exceeds fact budget for {}",
                self.subject
            );
            let Term::NamedNode(predicate) = &row["predicate"] else {
                anyhow::bail!("RDF predicate must be named")
            };
            facts.push((predicate.clone(), row["object"].clone()));
        }
        ensure!(
            !facts.is_empty(),
            "anonymous RDF expression has no retained facts for {}",
            self.subject
        );
        Ok(facts)
    }

    async fn source_collection(
        &self,
        view: &RevisionView,
        mut facts: Vec<(NamedNode, Term)>,
        references: &mut HashSet<NamedNode>,
        path: &mut HashSet<NamedNode>,
        depth: usize,
    ) -> Result<String> {
        const FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
        const REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
        const NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
        let mut items = Vec::new();
        let mut cells = Vec::new();
        loop {
            ensure!(
                items.len() < 4096,
                "RDF collection exceeds 4096-item budget for {}",
                self.subject
            );
            let first: Vec<_> =
                facts.iter().filter(|(p, _)| p.as_str() == FIRST).collect();
            let rest: Vec<_> =
                facts.iter().filter(|(p, _)| p.as_str() == REST).collect();
            ensure!(
                first.len() == 1 && rest.len() == 1,
                "malformed RDF collection for {}: each cell needs exactly one \
                 rdf:first and rdf:rest",
                self.subject
            );
            let mut item = self
                .source_value(view, &first[0].1, references, path, depth + 1)
                .await?;
            let Term::NamedNode(next) = &rest[0].1 else {
                anyhow::bail!(
                    "malformed RDF collection for {}: rdf:rest must identify \
                     a list cell or rdf:nil",
                    self.subject
                )
            };
            let next = next.clone();
            let mut annotations = Vec::new();
            for (predicate, object) in &facts {
                if predicate.as_str() == FIRST ||
                    predicate.as_str() == REST ||
                    (predicate.as_str() == RDF_TYPE &&
                        matches!(object, Term::NamedNode(n) if n.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#List"))
                {
                    continue;
                }
                let value = self
                    .source_value(view, object, references, path, depth + 1)
                    .await?;
                annotations.push(format!(
                    "{}: {value}",
                    code(&self.compact(predicate))
                ));
            }
            if !annotations.is_empty() {
                item.push_str(&format!(
                    " (list-cell annotations: {})",
                    annotations.join("; ")
                ));
            }
            items.push(item);
            if next.as_str() == NIL {
                break;
            }
            ensure!(
                path.insert(next.clone()),
                "cyclic RDF collection for {}",
                self.subject
            );
            cells.push(next.clone());
            facts = self.anonymous_facts(view, &next).await?;
        }
        for cell in cells {
            path.remove(&cell);
        }
        Ok(format!("({})", items.join(", ")))
    }

    fn source_value<'a>(
        &'a self,
        view: &'a RevisionView,
        value: &'a Term,
        references: &'a mut HashSet<NamedNode>,
        path: &'a mut HashSet<NamedNode>,
        depth: usize,
    ) -> futures::future::BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            ensure!(
                depth <= 24,
                "anonymous RDF expression exceeds depth budget for {}",
                self.subject
            );
            match value {
                Term::NamedNode(iri)
                    if iri.as_str().starts_with("urn:grapharch:skolem:") =>
                {
                    ensure!(
                        path.insert(iri.clone()),
                        "cyclic anonymous RDF expression for {}",
                        self.subject
                    );
                    let facts = self.anonymous_facts(view, iri).await?;
                    let rendered = if facts.iter().any(|(p, _)| {
                        matches!(p.as_str(),
                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#first" |
                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest")
                    }) {
                        self.source_collection(
                            view, facts, references, path, depth,
                        )
                        .await?
                    } else {
                        let mut pairs = Vec::new();
                        for (predicate, object) in facts {
                            let object = self
                                .source_value(
                                    view,
                                    &object,
                                    references,
                                    path,
                                    depth + 1,
                                )
                                .await?;
                            pairs.push(format!(
                                "{}: {object}",
                                code(&self.compact(&predicate))
                            ));
                        }
                        format!("({})", pairs.join("; "))
                    };
                    path.remove(iri);
                    Ok(rendered)
                },
                Term::NamedNode(iri)
                    if iri.as_str() ==
                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil" =>
                {
                    Ok("()".into())
                },
                Term::NamedNode(iri) if self.documented.contains(iri) => {
                    references.insert(iri.clone());
                    Ok(format!(
                        "[{}]({})",
                        escape(&self.compact(iri)),
                        iri.as_str()
                    ))
                },
                Term::NamedNode(iri) => Ok(code(&self.compact(iri))),
                Term::Literal(value) => Ok(escape(value.value())),
                _ => anyhow::bail!("unsupported source RDF value"),
            }
        })
    }

    /// Stable fragment identity, independent of output format and placement.
    pub fn fragment_id(&self) -> NamedNode {
        identity("annotated-term-fragment", &[
            self.project.as_str(),
            self.subject.as_str(),
            &self.language,
        ])
    }

    fn compact(&self, iri: &NamedNode) -> String {
        for (namespace, prefix) in self.prefixes.iter() {
            if let Some(local) = iri.as_str().strip_prefix(namespace) {
                return format!("{prefix}:{local}");
            }
        }
        iri.as_str().into()
    }

    async fn union(
        &self,
        view: &RevisionView,
        predicate: &NamedNode,
        value: &Term,
        references: &mut HashSet<NamedNode>,
    ) -> Result<String> {
        // Anchor through the named subject: SPARQL blank-node labels are
        // patterns, not identifiers of stored blank nodes.
        let pattern = view.pattern(&format!("{} {predicate} ?root . ?root <http://www.w3.org/2002/07/owl#unionOf> ?list . ?list <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>*/<http://www.w3.org/1999/02/22-rdf-syntax-ns#first> ?member", self.subject), 2);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?root ?member WHERE {{ {pattern} }} ORDER BY \
                 ?member LIMIT 65"
            ))
            .await?;
        let mut members = Vec::new();
        while let Some(row) = rows.next().await? {
            if &row["root"] != value {
                continue;
            }
            ensure!(
                members.len() < 64,
                "OWL union exceeds member budget"
            );
            let Term::NamedNode(iri) = &row["member"] else {
                anyhow::bail!("OWL union members must be named resources")
            };
            members.push(
                if self.documented.contains(iri) {
                    references.insert(iri.clone());
                    format!(
                        "[{}]({})",
                        escape(&self.compact(iri)),
                        iri.as_str()
                    )
                } else {
                    code(&self.compact(iri))
                },
            );
        }
        ensure!(
            !members.is_empty(),
            "unsupported anonymous RDF expression on {} {predicate}",
            self.subject
        );
        Ok(format!("union of {}", members.join(", ")))
    }

    fn selected(&self, literal: &Literal) -> bool {
        super::description::selected(literal, &self.language)
    }
}
#[async_trait]
impl Documentor for AnnotatedTermDocumentor {
    async fn document(&self, writer: &GraphWriter) -> Result<()> {
        let view =
            RevisionView::new(self.dataset.clone(), self.snapshot.clone());
        let pattern =
            view.pattern(&format!("{} ?predicate ?object", self.subject), 0);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?predicate ?object WHERE {{ {pattern} }} \
                 ORDER BY ?predicate ?object"
            ))
            .await?;
        let mut label = Vec::new();
        let mut preferred_labels = Vec::new();
        let mut definitions = Vec::new();
        let mut markdown_descriptions = Vec::new();
        let mut comments = Vec::new();
        let mut notes = Vec::new();
        let mut facts = Vec::new();
        let mut types = Vec::new();
        let mut references = HashSet::new();
        let mut hash = Sha256::new();
        let mut examples = Vec::new();
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            ensure!(
                count < 512,
                "annotated term exceeds 512-fact budget"
            );
            count += 1;
            let Term::NamedNode(predicate) = &row["predicate"] else {
                anyhow::bail!("term predicate must be an IRI")
            };
            let value = &row["object"];
            hash.update(format!("{predicate} {value}\n"));
            let values = match predicate.as_str() {
                "http://www.w3.org/2000/01/rdf-schema#label" => {
                    Some(&mut label)
                },
                "http://www.w3.org/2004/02/skos/core#prefLabel" => {
                    Some(&mut preferred_labels)
                },
                super::description::MARKDOWN => {
                    Some(&mut markdown_descriptions)
                },
                "http://purl.org/dc/terms/description" => {
                    Some(&mut definitions)
                },
                "http://www.w3.org/2004/02/skos/core#definition" => {
                    if self.source_annotations {
                        Some(&mut definitions)
                    } else {
                        continue;
                    }
                },
                "http://www.w3.org/2004/02/skos/core#example" => {
                    let Term::Literal(value) = value else {
                        anyhow::bail!("RDF example must be a literal")
                    };
                    if self.selected(value) {
                        examples.push(value.value().to_owned());
                    }
                    continue;
                },
                "http://www.w3.org/2004/02/skos/core#scopeNote" |
                "http://www.w3.org/2004/02/skos/core#note" => Some(&mut notes),
                "http://www.w3.org/2000/01/rdf-schema#comment" => {
                    if self.source_annotations {
                        Some(&mut definitions)
                    } else {
                        Some(&mut comments)
                    }
                },
                _ => None,
            };
            if let Some(values) = values {
                let Term::Literal(value) = value else {
                    anyhow::bail!("term annotation must be a literal")
                };
                if self.selected(value) {
                    values.push(value.value().to_owned());
                }
                continue;
            }
            let value = if self.source_annotations {
                self.source_value(
                    &view,
                    value,
                    &mut references,
                    &mut HashSet::new(),
                    0,
                )
                .await?
            } else {
                match value {
                    Term::NamedNode(iri) if self.documented.contains(iri) => {
                        references.insert(iri.clone());
                        format!(
                            "[{}]({})",
                            escape(&self.compact(iri)),
                            iri.as_str()
                        )
                    },
                    Term::NamedNode(iri)
                        if iri
                            .as_str()
                            .starts_with("urn:grapharch:skolem:") =>
                    {
                        self.union(&view, predicate, value, &mut references)
                            .await?
                    },
                    Term::NamedNode(iri) => code(&self.compact(iri)),
                    Term::Literal(value) => {
                        let suffix =
                            if let Some(language) = value.language() {
                                format!(" (@{})", escape(language))
                            } else if value.datatype().as_str() !=
                                "http://www.w3.org/2001/XMLSchema#string"
                            {
                                format!(
                                    " ({})",
                                    code(&self.compact(
                                        &value.datatype().into_owned()
                                    ))
                                )
                            } else {
                                String::new()
                            };
                        format!("{}{}", escape(value.value()), suffix)
                    },
                    Term::BlankNode(_) => {
                        self.union(&view, predicate, value, &mut references)
                            .await?
                    },
                }
            };
            if predicate.as_str() == RDF_TYPE &&
                let Term::NamedNode(iri) = &row["object"]
            {
                types.push((iri.clone(), value.clone()));
            }
            facts.push(format!(
                "| {} | {value} |",
                code(&self.compact(predicate))
            ));
        }
        ensure!(
            preferred_labels.len() <= 1,
            "{} has ambiguous skos:prefLabel annotations for {}",
            self.subject,
            self.language
        );
        if !preferred_labels.is_empty() {
            label = preferred_labels;
        }
        let markdown = super::description::select(
            markdown_descriptions,
            &self.subject,
            &self.language,
        )?;
        if markdown.is_none() {
            notes.extend(comments);
        }
        ensure!(
            self.source_annotations || label.len() == 1,
            "{} requires exactly one label for {}",
            self.subject,
            self.language
        );
        ensure!(
            markdown.is_some() ||
                self.source_annotations ||
                definitions.len() == 1,
            "{} requires exactly one Dublin Core description for {}",
            self.subject,
            self.language
        );
        let missing_label = label.is_empty();
        let missing_definition = markdown.is_none() && definitions.is_empty();
        let mut distinct_definitions = HashSet::new();
        definitions.retain(|definition| {
            distinct_definitions.insert(definition.clone())
        });
        let label_line = if label.is_empty() {
            String::new()
        } else {
            format!(
                "**Label:** {}",
                label
                    .iter()
                    .map(|value| escape(value))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };
        let definition_text = match markdown {
            Some(body) => {
                super::description::nested(
                    &body,
                    if self.family_reference { 4 } else { 3 },
                )?
            },
            None => {
                definitions
                    .iter()
                    .map(|value| escape(value))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            },
        };
        let mut gaps = Vec::new();
        if self.source_annotations && missing_label {
            gaps.push("Missing source label.");
        }
        if self.source_annotations && missing_definition {
            gaps.push(
                "Missing source definition. This source declares the resource \
                 without an English definition.",
            );
        }
        let gaps = if gaps.is_empty() {
            String::new()
        } else {
            format!("\n\n> [!WARNING]\n> {}\n", gaps.join(" "))
        };
        let individual = types.iter().any(|(iri, _)| {
            iri.as_str() == "http://www.w3.org/2002/07/owl#NamedIndividual"
        });
        let heading = if self.family_reference {
            if label.is_empty() {
                self.compact(&self.subject)
            } else {
                label.join("; ")
            }
        } else if individual {
            let name = if label.is_empty() {
                self.compact(&self.subject)
            } else {
                label.join("; ")
            };
            format!("Named individual: {name}")
        } else {
            self.compact(&self.subject)
        };
        let mut individual_summary = String::new();
        if self.family_reference && !individual {
            individual_summary.push_str(&format!(
                "\n\n**Identifier:** {}",
                code(&self.compact(&self.subject))
            ));
        }
        if individual {
            let (roles, classes): (Vec<_>, Vec<_>) =
                types
                    .iter()
                    .filter(|(iri, _)| {
                        iri.as_str() !=
                            "http://www.w3.org/2002/07/owl#NamedIndividual"
                    })
                    .partition(|(iri, _)| {
                        matches!(iri.as_str(),
                    "http://www.w3.org/2002/07/owl#Class" |
                    "http://www.w3.org/2000/01/rdf-schema#Class" |
                    "http://www.w3.org/2000/01/rdf-schema#Datatype" |
                    "http://www.w3.org/2002/07/owl#ObjectProperty" |
                    "http://www.w3.org/2002/07/owl#DatatypeProperty" |
                    "http://www.w3.org/2002/07/owl#AnnotationProperty")
                    });
            individual_summary.push_str(&format!(
                "\n\n**Identifier:** {}",
                code(&self.compact(&self.subject))
            ));
            if !classes.is_empty() {
                individual_summary.push_str(&format!(
                    "\n\n**Instance of:** {}",
                    classes
                        .iter()
                        .map(|(_, rendered)| rendered.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !roles.is_empty() {
                individual_summary.push_str(&format!(
                    "\n\n**Also declared as:** {}",
                    roles
                        .iter()
                        .map(|(_, rendered)| rendered.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let body = format!(
            "{} {}\n\n{label_line}{individual_summary}\n\n{}{}\n\n**IRI:** \
             {}\n\n{}\n\n| RDF predicate | Value |\n| --- | --- |\n{}\n",
            if self.family_reference { "###" } else { "##" },
            escape(&heading),
            definition_text,
            gaps,
            code(self.subject.as_str()),
            notes
                .iter()
                .map(|note| format!("**Note:** {}", escape(note)))
                .collect::<Vec<_>>()
                .join("\n\n"),
            facts.join("\n")
        );
        let mut body = body;
        for example in examples {
            let width = example
                .split(|c| c != '`')
                .map(str::len)
                .max()
                .unwrap_or(0)
                .max(2) +
                1;
            let fence = "`".repeat(width);
            body.push_str(&format!(
                "\n\n**Example:**\n\n{fence}turtle\n{example}\n{fence}\n"
            ));
        }
        ensure!(
            body.len() <= 256 * 1024,
            "annotated term fragment exceeds byte budget"
        );
        hash.update([u8::from(self.source_annotations)]);
        hash.update(body.as_bytes());
        let revision = format!(
            "annotated-term-v6:{}:{}",
            self.language,
            hex::encode(hash.finalize())
        );
        let processor =
            Self::processor_identity(self.project.as_str(), &self.language);
        let contribution = identity("contribution", &[
            self.project.as_str(),
            processor.as_str(),
            self.subject.as_str(),
        ]);
        let mut stream = writer
            .begin_stage(
                self.project.as_str(),
                processor.as_str(),
                self.subject.as_str(),
                &revision,
                Stage::Document,
            )
            .await?;
        let fragment = self.fragment_id();
        let metadata: [(NamedNode, NamedNode, Term); 11] = [
            (
                processor.clone(),
                NamedNode::new_unchecked(RDF_TYPE),
                term("Documentor").into(),
            ),
            (
                contribution.clone(),
                NamedNode::new_unchecked(RDF_TYPE),
                term("Contribution").into(),
            ),
            (
                contribution.clone(),
                term("project"),
                self.project.clone().into(),
            ),
            (
                contribution.clone(),
                term("processor"),
                processor.clone().into(),
            ),
            (
                contribution.clone(),
                term("source"),
                Literal::new_simple_literal(self.subject.as_str()).into(),
            ),
            (
                contribution.clone(),
                term("revision"),
                Literal::new_simple_literal(revision).into(),
            ),
            (
                fragment.clone(),
                NamedNode::new_unchecked(RDF_TYPE),
                term("Fragment").into(),
            ),
            (
                fragment.clone(),
                term("subject"),
                self.subject.clone().into(),
            ),
            (
                fragment.clone(),
                term("body"),
                Literal::new_simple_literal(body).into(),
            ),
            (
                fragment.clone(),
                term("format"),
                Literal::new_simple_literal("text/markdown").into(),
            ),
            (
                fragment.clone(),
                term("contribution"),
                contribution.into(),
            ),
        ];
        for (subject, predicate, object) in metadata {
            stream
                .write(Quad::new(
                    subject,
                    predicate,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        if self.family_reference {
            stream
                .write(Quad::new(
                    fragment.clone(),
                    term("headingDepth"),
                    Literal::from(2u32),
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        for reference in references {
            stream
                .write(Quad::new(
                    fragment.clone(),
                    term("reference"),
                    reference,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        stream.complete().await?;
        if self.source_annotations {
            for (missing, code, message) in [
                (
                    missing_label,
                    "missing-source-label",
                    "Missing English source label",
                ),
                (
                    missing_definition,
                    "missing-source-definition",
                    "Missing English source definition",
                ),
            ] {
                if missing {
                    writer.record_diagnostic_at(crate::publication::diagnostics::SourceContext { project:self.project.as_str(),processor:processor.as_str(),source:self.subject.as_str(),stage:"document",input_revision:Some(&self.snapshot) },crate::publication::diagnostics::Diagnostic { severity: crate::publication::diagnostics::Severity::Warning, code: code.into(),message:message.into(),reference:self.subject.as_str().into(),source_fragment:Some(self.fragment_id().into_string()) }).await?;
                }
            }
        }
        Ok(())
    }
}
fn escape(text: &str) -> String {
    let mut result = String::new();
    for c in text.chars() {
        if "\\`*_[]<>|".contains(c) {
            result.push('\\');
        }
        if c == '\n' || c == '\r' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}
fn code(text: &str) -> String {
    let width = text.split(|c| c != '`').map(str::len).max().unwrap_or(0) + 1;
    let fence = "`".repeat(width);
    format!(
        "{fence} {} {fence}",
        text.replace('|', "\\|").replace(['\n', '\r'], " ")
    )
}
