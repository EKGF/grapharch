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
};

pub(super) struct Plan {
    pub dataset:     Dataset,
    pub handle:      ResolvedPlan,
    pub source:      RevisionView,
    pub project:     NamedNode,
    pub target:      NamedNode,
    pub revision:    NamedNode,
    pub scope:       NamedNode,
    pub profile:     NamedNode,
    pub renderer:    String,
    pub destination: String,
    pub title:       String,
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
    /// Files owned by this generation; bounded by the renderer's fixed
    /// artifact set.
    pub files:          Vec<String>,
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
        Ok(Self {
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
        })
    }

    pub fn is_html(&self) -> bool {
        matches!(self.renderer.as_str(), "html" | "specification")
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

    pub fn manifest(&self, job: &str) -> PublicationManifest {
        // bounded: the fixed output set; no per-subject artifact list is
        // collected.
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
            diagnostics: Vec::new(),
            failure_mode: Default::default(),
        }
    }
}

pub(super) async fn title(
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
        let subjects = self
            .source
            .pattern(&format!("?fragment {} {iri}", term("subject")), 2);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT ?fragment ?placement ?order ?fragmentOrder WHERE {{ \
                 {entries} {fields} {subjects} }} ORDER BY ?order \
                 ?fragmentOrder ?entry LIMIT 1"
            ))
            .await?;
        if let Some(row) = rows.next().await? {
            return Ok(format!(
                "#{}",
                self.anchor(
                    node(&row["placement"])?,
                    Some(node(&row["fragment"])?)
                )
            ));
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
                    "status"
            ),
            "unsupported section kind {kind}"
        );
        Ok(kind)
    }
}
