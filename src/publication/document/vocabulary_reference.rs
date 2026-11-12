//! A configured vocabulary reference: authored reference sections select
//! collected terms by type, and each selected term becomes one annotated term
//! fragment of its section's authored component. Placement stays in the
//! publication definitions.
use {
    super::{
        AnnotatedTermDocumentor,
        Documentor,
        namespace_bindings::NamespaceBindings,
        plantuml::LocalRenderer,
        scope_inclusion,
        skos::SchemeArrangement,
    },
    crate::publication::{
        dataset::{Dataset, Stage},
        limits::FAMILY_RESOURCES,
        term,
        validation::unsigned,
        view::RevisionView,
        writer::{ContributionStream, GraphWriter},
    },
    anyhow::{Context, Result, bail, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    std::{collections::HashSet, sync::Arc},
};

const SECTIONS: usize = 16;
const SKOS_CONCEPT: &str = "http://www.w3.org/2004/02/skos/core#Concept";

/// Generates the term fragments of one configured `ga:DocumentorUse`, using
/// the source annotations of the collected vocabulary.
pub struct VocabularyReferenceDocumentor {
    dataset:     Dataset,
    definitions: RevisionView,
    collected:   NamedNode,
    project:     NamedNode,
    processor:   NamedNode,
}

/// One authored reference section with the terms it documents.
struct Section {
    node:         NamedNode,
    order:        u32,
    term_type:    NamedNode,
    component:    NamedNode,
    defined_only: bool,
    terms:        Vec<NamedNode>,
}

impl VocabularyReferenceDocumentor {
    /// Bind a documentor use from a definitions-only view to the project's
    /// collected-stage snapshot, never to generated documentation.
    pub async fn new(
        dataset: Dataset,
        definitions: RevisionView,
        project: &NamedNode,
        processor: NamedNode,
    ) -> Result<Self> {
        let collected =
            dataset.snapshot_collected_for(project.as_str()).await?;
        Ok(Self {
            dataset,
            definitions,
            collected,
            project: project.clone(),
            processor,
        })
    }

    async fn sections(&self) -> Result<Vec<Section>> {
        let mut rows = self
            .definitions
            .objects(&self.processor, "referenceSection")
            .await?;
        let mut sections = Vec::new(); // bounded: SECTIONS.
        while let Some(row) = rows.next().await? {
            ensure!(
                sections.len() < SECTIONS,
                "{} exceeds the {SECTIONS}-section budget",
                self.processor
            );
            let Term::NamedNode(node) = &row["value"] else {
                bail!("ga:referenceSection must identify a ga:ReferenceSection")
            };
            sections.push(
                self.section(node)
                    .await
                    .with_context(|| format!("reference section {node}"))?,
            );
        }
        ensure!(
            !sections.is_empty(),
            "{} declares no ga:referenceSection",
            self.processor
        );
        sections.sort_by_key(|section| section.order);
        for pair in sections.windows(2) {
            ensure!(
                pair[0].order != pair[1].order,
                "reference sections {} and {} have ambiguous order {}",
                pair[0].node,
                pair[1].node,
                pair[0].order
            );
        }
        for (index, section) in sections.iter().enumerate() {
            for other in &sections[index + 1..] {
                ensure!(
                    section.term_type != other.term_type,
                    "reference sections {} and {} select the same ga:termType",
                    section.node,
                    other.node
                );
                ensure!(
                    section.component != other.component,
                    "reference sections {} and {} fill the same ga:component",
                    section.node,
                    other.node
                );
            }
        }
        Ok(sections)
    }

    async fn section(&self, node: &NamedNode) -> Result<Section> {
        let definitions = &self.definitions;
        ensure!(
            definitions.has_type(node, "ReferenceSection").await?,
            "must be typed ga:ReferenceSection"
        );
        let order = definitions
            .optional(node, "order")
            .await?
            .context("requires ga:order")?;
        let order = unsigned(&order, "reference section order")?;
        ensure!(order <= 1000, "ga:order exceeds 1000");
        let component = definitions.node(node, "component").await?;
        ensure!(
            definitions.has_type(&component, "Component").await?,
            "ga:component {component} must be an authored ga:Component"
        );
        let defined_only =
            match definitions.optional(node, "definedTermsOnly").await? {
                None => false,
                Some(Term::Literal(value))
                    if value.datatype().as_str() ==
                        "http://www.w3.org/2001/XMLSchema#boolean" =>
                {
                    matches!(value.value(), "true" | "1")
                },
                Some(_) => bail!("ga:definedTermsOnly must be an xsd:boolean"),
            };
        Ok(Section {
            node: node.clone(),
            order,
            term_type: definitions.node(node, "termType").await?,
            component,
            defined_only,
            terms: Vec::new(),
        })
    }

    /// Select each term once, in the lowest-ordered section whose type it
    /// declares.
    async fn select(
        &self,
        view: &RevisionView,
        sections: &mut [Section],
    ) -> Result<HashSet<NamedNode>> {
        let mut known = HashSet::new(); // bounded: FAMILY_RESOURCES.
        for section in sections.iter_mut() {
            let pattern =
                view.pattern(&format!("?subject a {}", section.term_type), 0);
            let ownership = if section.defined_only {
                format!(
                    "{} {}",
                    view.pattern(
                        "?subject \
                         <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> \
                         ?ontology",
                        1
                    ),
                    view.pattern(
                        "?ontology a <http://www.w3.org/2002/07/owl#Ontology>",
                        2
                    )
                )
            } else {
                String::new()
            };
            let mut rows = self
                .dataset
                .select(format!(
                    "SELECT DISTINCT ?subject WHERE {{ {pattern} {ownership} \
                     }} ORDER BY ?subject LIMIT {}",
                    FAMILY_RESOURCES + 1
                ))
                .await?;
            while let Some(row) = rows.next().await? {
                let Term::NamedNode(subject) = &row["subject"] else {
                    bail!(
                        "vocabulary reference terms require named identities: \
                         {}",
                        row["subject"]
                    )
                };
                if known.insert(subject.clone()) {
                    ensure!(
                        known.len() <= FAMILY_RESOURCES,
                        "vocabulary reference exceeds the \
                         {FAMILY_RESOURCES}-term budget"
                    );
                    section.terms.push(subject.clone());
                }
            }
        }
        Ok(known)
    }

    /// Place SKOS concepts after their canonical scheme; concepts without a
    /// documented scheme belong to the `skos:Concept` section.
    fn arrange(
        &self,
        skos: &SchemeArrangement,
        sections: &mut [Section],
    ) -> Result<()> {
        for section in sections.iter_mut() {
            let mut arranged = Vec::new(); // bounded: the selected terms.
            for subject in std::mem::take(&mut section.terms) {
                if skos.is_concept(&subject) {
                    continue;
                }
                arranged.push(subject.clone());
                if skos.is_scheme(&subject) {
                    arranged.extend(skos.canonical_members(&subject));
                }
            }
            section.terms = arranged;
        }
        let unassigned = skos.unassigned();
        if !unassigned.is_empty() {
            let section = sections
                .iter_mut()
                .find(|section| section.term_type.as_str() == SKOS_CONCEPT)
                .with_context(|| {
                    format!(
                        "{} documents {} SKOS concepts without a documented \
                         scheme, but no reference section has ga:termType \
                         skos:Concept",
                        self.processor,
                        unassigned.len()
                    )
                })?;
            section.terms.extend(unassigned);
        }
        Ok(())
    }

    async fn bind(
        &self,
        stream: &mut ContributionStream,
        writer: &GraphWriter,
        sections: &[Section],
        skos: &SchemeArrangement,
        known: Arc<HashSet<NamedNode>>,
    ) -> Result<()> {
        let view =
            RevisionView::new(self.dataset.clone(), self.collected.clone());
        let ontology =
            self.definitions.node(&self.processor, "subject").await?;
        let scopes =
            scope_inclusion::scopes(&self.definitions, &self.processor).await?;
        let prefixes = Arc::new(
            NamespaceBindings::well_known()
                .read_declarations(&self.dataset, &view)
                .await?
                .into_map(),
        );
        let renderer = Arc::new(LocalRenderer::new(std::ffi::OsStr::new(
            "plantuml",
        )));
        for section in sections {
            for (position, subject) in section.terms.iter().enumerate() {
                let documentor = AnnotatedTermDocumentor::new(
                    self.dataset.clone(),
                    self.collected.clone(),
                    self.project.as_str(),
                    subject.clone(),
                    "en",
                    prefixes.clone(),
                    known.clone(),
                )?
                .with_diagram_renderer(renderer.clone())
                .with_ontology_context(ontology.clone())
                .with_arrangement(skos.decoration(subject, 1))
                .with_source_annotations();
                documentor.document(writer).await?;
                let fragment = documentor.fragment_id();
                let mut quads = vec![
                    (
                        section.component.clone(),
                        term("fragment"),
                        Term::from(fragment.clone()),
                    ),
                    (
                        fragment,
                        term("order"),
                        Literal::from(u32::try_from(position)?).into(),
                    ),
                ];
                for scope in &scopes {
                    quads.push((
                        scope.clone(),
                        term("include"),
                        subject.clone().into(),
                    ));
                }
                for (subject, predicate, object) in quads {
                    stream
                        .write(Quad::new(
                            subject,
                            predicate,
                            object,
                            GraphName::DefaultGraph,
                        ))
                        .await?;
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Documentor for VocabularyReferenceDocumentor {
    async fn document(&self, writer: &GraphWriter) -> Result<()> {
        let mut sections = self.sections().await?;
        let view =
            RevisionView::new(self.dataset.clone(), self.collected.clone());
        let known = self.select(&view, &mut sections).await?;
        let skos =
            SchemeArrangement::read(&self.dataset, &view, &known).await?;
        self.arrange(&skos, &mut sections)?;
        let mut stream = writer
            .begin_stage(
                self.project.as_str(),
                self.processor.as_str(),
                self.processor.as_str(),
                &format!("vocabulary-reference-v1:{}", self.collected),
                Stage::Document,
            )
            .await?;
        match self
            .bind(
                &mut stream,
                writer,
                &sections,
                &skos,
                Arc::new(known),
            )
            .await
        {
            Ok(()) => stream.complete().await,
            Err(error) => {
                stream.abort().await?;
                Err(error)
            },
        }
    }
}

#[cfg(test)]
mod tests;
