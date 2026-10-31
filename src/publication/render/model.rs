use {
    crate::publication::{
        assembly::Assembler,
        dataset::{Dataset, Rows, identity},
        plan::ResolvedPlan,
        term,
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

pub(super) struct Plan {
    pub dataset:          Dataset,
    pub handle:           ResolvedPlan,
    pub source:           RevisionView,
    pub project:          NamedNode,
    pub target:           NamedNode,
    pub revision:         NamedNode,
    pub scope:            NamedNode,
    pub profile:          NamedNode,
    pub renderer:         String,
    pub destination:      String,
    pub title:            String,
    link_index:           HashMap<NamedNode, String>,
    pub images:           super::collected_images::Images,
    pub quality_evidence: super::quality::EvidenceIndex,
    pub quality:          Vec<crate::publication::diagnostics::Diagnostic>,
}

/// Per-book result carried with its parent's atomic publication manifest.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BookPublication {
    /// Resolved BookPlan identity.
    pub plan:       String,
    /// Module or ontology identity.
    pub subject:    String,
    /// Relative HTML entry point.
    pub html:       String,
    /// Relative PDF route when generation succeeded.
    pub pdf:        Option<String>,
    /// Execution state, ready or failed.
    pub status:     String,
    /// Whether a verified previously generated PDF was reused.
    pub reused:     bool,
    /// Fingerprint of the effective compiler input.
    pub digest:     String,
    /// SHA-256 of published PDF bytes.
    pub pdf_digest: Option<String>,
}

/// Provenance and fixed artifact ownership for one completed publication.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationManifest {
    /// Manifest format version.
    pub version:        u32,
    /// Project identity.
    pub project:        String,
    /// Target identity.
    pub target:         String,
    /// Validated source revision.
    pub input_revision: String,
    /// Immutable publication plan.
    pub plan:           String,
    /// Pinned view of the completed plan.
    pub plan_snapshot:  String,
    /// Scope definition.
    pub scope:          String,
    /// Publication profile.
    pub profile:        String,
    /// Output engine.
    pub renderer:       String,
    /// Successful job identity.
    pub job:            String,
    /// Files owned by this generation; bounded by the renderer's
    /// artifact budgets.
    pub files:          Vec<String>,
    /// Results of independently rendered books, pinned to this publication.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub books:          Vec<BookPublication>,
    /// Execution policy used for this publication.
    #[serde(default)]
    pub failure_mode:   crate::publication::diagnostics::FailureMode,
    /// Errors in an explicitly requested keep-going preview.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics:    Vec<crate::publication::diagnostics::Diagnostic>,
}

impl Plan {
    pub async fn read(dataset: Dataset, handle: ResolvedPlan) -> Result<Self> {
        let view = RevisionView::new(dataset.clone(), handle.snapshot.clone());
        ensure!(
            view.has_type(&handle.id, "Plan").await?,
            "missing publication plan {}",
            handle.id
        );
        let project = view.node(&handle.id, "project").await?;
        let target = view.node(&handle.id, "target").await?;
        let revision = view.node(&handle.id, "inputRevision").await?;
        Assembler::new(dataset.clone())
            .require_validated(project.as_str(), &revision)
            .await?;
        ensure!(
            handle.id ==
                identity("publication-plan", &[
                    project.as_str(),
                    target.as_str(),
                    revision.as_str()
                ]),
            "publication plan identity disagrees with its provenance"
        );
        let source = RevisionView::new(dataset.clone(), revision.clone());
        let scope = view.node(&handle.id, "scope").await?;
        let profile = view.node(&handle.id, "profile").await?;
        let renderer = view.text(&handle.id, "renderer").await?;
        let destination = view.text(&handle.id, "destination").await?;
        ensure!(
            source.node(&target, "scope").await? == scope &&
                source.node(&target, "profile").await? == profile &&
                source.text(&target, "renderer").await? == renderer &&
                source.text(&target, "destination").await? == destination,
            "plan does not match its pinned target definition"
        );
        let title = title(&source, &target).await?;
        let (quality, quality_evidence) =
            super::quality::read(&view, &handle.id, &dataset).await?;
        let mut plan = Self {
            dataset,
            handle,
            source,
            project,
            target,
            revision,
            scope,
            profile,
            renderer,
            destination,
            title,
            link_index: HashMap::new(),
            images: Default::default(),
            quality,
            quality_evidence,
        };
        if plan.renderer != "website" {
            plan.index_links().await?;
        }
        Ok(plan)
    }

    pub async fn toc_depth(&self) -> Result<Option<usize>> {
        let value = match self.source.optional(&self.target, "tocDepth").await?
        {
            Some(value) => Some(value),
            None => self.source.optional(&self.profile, "tocDepth").await?,
        };
        let Some(value) = value else { return Ok(None) };
        let Term::Literal(value) = value else {
            anyhow::bail!("tocDepth must be an integer")
        };
        ensure!(
            value.datatype().as_str() ==
                "http://www.w3.org/2001/XMLSchema#integer",
            "tocDepth must have datatype xsd:integer"
        );
        let depth = value.value().parse::<usize>()?;
        ensure!(
            (1..=8).contains(&depth),
            "tocDepth must be between one and eight"
        );
        Ok(Some(depth))
    }

    pub fn is_html(&self) -> bool {
        matches!(
            self.renderer.as_str(),
            "html" | "specification" | "specification-pages" | "bookshelf"
        )
    }

    pub async fn entries(&self) -> Result<Rows> {
        let entries = Dataset::logical_pattern(
            &self.handle.snapshot,
            &format!("{} {} ?entry", self.handle.id, term("entry")),
            0,
        );
        let fields = Dataset::logical_pattern(
            &self.handle.snapshot,
            &format!(
                "?entry {} ?fragment ; {} ?component ; {} ?placement ; {} \
                 ?order ; {} ?fragmentOrder",
                term("fragment"),
                term("component"),
                term("placement"),
                term("order"),
                term("fragmentOrder")
            ),
            1,
        );
        self.dataset
            .select(format!(
                "SELECT ?entry ?fragment ?component ?placement ?order \
                 ?fragmentOrder WHERE {{ {entries} {fields} }} ORDER BY \
                 ?order ?fragmentOrder ?entry"
            ))
            .await
    }

    pub async fn assets(&self) -> Result<Rows> {
        let pattern = Dataset::logical_pattern(
            &self.handle.snapshot,
            &format!("{} {} ?asset", self.handle.id, term("asset")),
            0,
        );
        self.dataset
            .select(format!(
                "SELECT DISTINCT ?asset WHERE {{ {pattern} }} ORDER BY ?asset"
            ))
            .await
    }

    pub fn manifest(&self, job: &str) -> PublicationManifest {
        // Fixed document outputs; bounded retained image routes are appended
        // after their bytes are restored and verified.
        let files = if self.renderer == "specification" {
            vec![
                "index.html".into(),
                "style.css".into(),
                "main.typ".into(),
                "document.pdf".into(),
            ]
        } else if self.is_html() {
            vec!["index.html".into(), "style.css".into()]
        } else {
            vec!["main.typ".into(), "document.pdf".into()]
        };
        PublicationManifest {
            version: 1,
            project: self.project.as_str().to_owned(),
            target: self.target.as_str().to_owned(),
            input_revision: self.revision.as_str().to_owned(),
            plan: self.handle.id.as_str().to_owned(),
            plan_snapshot: self.handle.snapshot.as_str().to_owned(),
            scope: self.scope.as_str().to_owned(),
            profile: self.profile.as_str().to_owned(),
            renderer: self.renderer.clone(),
            job: job.into(),
            files,
            books: Vec::new(),
            diagnostics: Vec::new(),
            failure_mode: Default::default(),
        }
    }
}

pub(crate) async fn title(
    view: &RevisionView,
    subject: &NamedNode,
) -> Result<String> {
    let predicate = NamedNode::new("http://purl.org/dc/terms/title")?;
    match view.optional_by(subject, &predicate).await? {
        Some(Term::Literal(title)) => Ok(title.value().to_owned()),
        None => Ok(subject.as_str().to_owned()),
        _ => anyhow::bail!("{subject} title must be a literal"),
    }
}

pub(super) fn node(term: &Term) -> Result<&NamedNode> {
    match term {
        Term::NamedNode(node) => Ok(node),
        _ => anyhow::bail!("plan entry reference must be an IRI"),
    }
}

impl Plan {
    pub fn anchor(
        &self,
        placement: &NamedNode,
        fragment: Option<&NamedNode>,
    ) -> String {
        identity("anchor", &[
            self.target.as_str(),
            placement.as_str(),
            fragment.map(NamedNode::as_str).unwrap_or("section"),
        ])
        .as_str()
        .replace(':', "-")
    }

    async fn index_links(&mut self) -> Result<()> {
        let entries = Dataset::logical_pattern(
            &self.handle.snapshot,
            &format!("{} {} ?entry", self.handle.id, term("entry")),
            0,
        );
        let fields = Dataset::logical_pattern(
            &self.handle.snapshot,
            &format!(
                "?entry {} ?fragment ; {} ?placement ; {} ?order ; {} \
                 ?fragmentOrder",
                term("fragment"),
                term("placement"),
                term("order"),
                term("fragmentOrder")
            ),
            1,
        );
        let subjects = self.source.pattern(
            &format!("?fragment {} ?subject", term("subject")),
            2,
        );
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?subject ?fragment ?placement ?order \
                 ?fragmentOrder ?entry WHERE {{ {entries} {fields} {subjects} \
                 }} ORDER BY ?order ?fragmentOrder ?entry"
            ))
            .await?;
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= crate::publication::limits::LINK_INDEX_ENTRIES,
                "publication link index exceeds {}-entry budget",
                crate::publication::limits::LINK_INDEX_ENTRIES
            );
            let subject = node(&row["subject"])?;
            let anchor = format!(
                "#{}",
                self.anchor(
                    node(&row["placement"])?,
                    Some(node(&row["fragment"])?)
                )
            );
            self.link_index.entry(subject.clone()).or_insert(anchor);
        }
        Ok(())
    }

    pub async fn link(
        &self,
        destination: &str,
        local_prefix: &str,
    ) -> Result<String> {
        if let Some(local) = destination.strip_prefix('#') {
            return Ok(if local.is_empty() {
                format!("#{local_prefix}")
            } else {
                format!("#{local_prefix}-h-{local}")
            });
        }
        let iri = NamedNode::new(destination).map_err(|_| {
            anyhow::anyhow!(
                "relative links require a collected asset or a subject IRI: \
                 {destination}"
            )
        })?;
        if let Some(anchor) = self.link_index.get(&iri) {
            return Ok(anchor.clone());
        }
        let destination = if self
            .source
            .has_type(&iri, "ExternalReference")
            .await?
        {
            match self.source.optional(&iri, "url").await? {
                Some(Term::Literal(url)) => url.value().to_owned(),
                None => destination.to_owned(),
                _ => anyhow::bail!("external reference URL must be a literal"),
            }
        } else {
            destination.to_owned()
        };
        let url = url::Url::parse(&destination)?;
        ensure!(
            matches!(url.scheme(), "http" | "https" | "mailto"),
            "unresolved or unsupported link: {destination}"
        );
        Ok(destination)
    }
}

impl Plan {
    pub async fn section_kind(&self, placement: &NamedNode) -> Result<String> {
        let kind = match self.source.optional(placement, "sectionKind").await? {
            None => "section".to_owned(),
            Some(Term::Literal(kind)) => kind.value().to_owned(),
            _ => anyhow::bail!("sectionKind must be a literal"),
        };
        ensure!(
            matches!(
                kind.as_str(),
                "section" |
                    "appendix" |
                    "glossary" |
                    "references" |
                    "abstract" |
                    "status" |
                    "preface"
            ),
            "unsupported section kind {kind}"
        );
        Ok(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn repeated_internal_links_do_not_repeat_graph_queries() -> Result<()>
    {
        let (dataset, plans) = crate::publication::showcases::specification(
            std::path::Path::new("examples/specification/project.ttl"),
        )
        .await?;
        let plan = Plan::read(dataset.clone(), plans[0].clone()).await?;
        let before = dataset
            .select_calls
            .load(std::sync::atomic::Ordering::Relaxed);
        let mut previous = None;
        for _ in 0..100 {
            let link =
                plan.link("urn:specification:subject", "section").await?;
            assert!(link.starts_with('#'));
            if let Some(previous) = &previous {
                assert_eq!(previous, &link);
            }
            previous = Some(link);
        }
        assert_eq!(
            dataset
                .select_calls
                .load(std::sync::atomic::Ordering::Relaxed),
            before,
            "links to pinned plan subjects must use the bounded index rather \
             than re-querying the whole plan"
        );
        Ok(())
    }
}
