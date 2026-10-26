//! Resolve target-specific placements from reusable components and scoped
//! content.

mod constraints;
use {
    super::{
        RDF_TYPE,
        assembly::Assembler,
        dataset::{Dataset, Stage, identity},
        scope::ScopeResolver,
        term,
        view::RevisionView,
        writer::{ContributionStream, GraphWriter},
    },
    anyhow::{Result, bail, ensure},
    constraints::{destination, destination_path, integer},
    oxrdf::{Literal, NamedNode, Quad, Term},
};

/// A planner bound to one validated input revision, independent of current
/// edits.
pub struct TargetPlanner {
    dataset:  Dataset,
    revision: NamedNode,
}

/// An immutable resolved plan, usable after later planning replaces the target
/// head.
#[derive(Clone, Debug)]
pub struct ResolvedPlan {
    /// Stable publication plan identity.
    pub id:       NamedNode,
    /// Snapshot of exactly the committed plan contribution.
    pub snapshot: NamedNode,
}

struct Selection<'a> {
    view:           &'a RevisionView,
    scope_snapshot: &'a NamedNode,
    scope_plan:     &'a NamedNode,
    plan:           &'a NamedNode,
    profile:        &'a NamedNode,
}

impl TargetPlanner {
    /// Bind all target definitions and fragment selections to this revision.
    pub fn new(dataset: Dataset, revision: NamedNode) -> Self {
        Self { dataset, revision }
    }

    /// Materialize a target plan only after scope, ownership, order, and
    /// references resolve.
    pub async fn resolve(
        &self,
        project: &str,
        target: &NamedNode,
        writer: &GraphWriter,
    ) -> Result<ResolvedPlan> {
        Assembler::new(self.dataset.clone())
            .require_validated(project, &self.revision)
            .await?;
        let view =
            RevisionView::new(self.dataset.clone(), self.revision.clone());
        let project = NamedNode::new(project)?;
        let registered = view.pattern(
            &format!("{project} {} {target}", term("target")),
            0,
        );
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT ?_owner0 WHERE {{ {registered} }} LIMIT 1"
            ))
            .await?;
        ensure!(
            rows.next().await?.is_some(),
            "target {target} is not registered by {project}"
        );
        drop(rows);
        self.check_destinations(&view, &project).await?;
        let scope = view.node(target, "scope").await?;
        let profile = view.node(target, "profile").await?;
        let renderer = view.text(target, "renderer").await?;
        let renderer_kind = super::Renderer::parse(&renderer)?;
        let destination = view.text(target, "destination").await?;
        let scope_plan =
            ScopeResolver::new(self.dataset.clone(), self.revision.clone())
                .resolve(project.as_str(), &scope, writer)
                .await?;
        let scope_snapshot = scope_plan.snapshot;
        let scope_plan = scope_plan.id;
        let plan = identity("publication-plan", &[
            project.as_str(),
            target.as_str(),
            self.revision.as_str(),
        ]);
        let mut stream = writer
            .begin_stage(
                project.as_str(),
                "urn:grapharch:target-planner:v1",
                target.as_str(),
                self.revision.as_str(),
                Stage::Plan,
            )
            .await?;
        for (predicate, object) in [
            (
                NamedNode::new_unchecked(RDF_TYPE),
                Term::NamedNode(term("Plan")),
            ),
            (term("project"), project.clone().into()),
            (term("target"), target.clone().into()),
            (term("scope"), scope.clone().into()),
            (term("profile"), profile.clone().into()),
            (term("scopePlan"), scope_plan.clone().into()),
            (
                term("inputRevision"),
                self.revision.clone().into(),
            ),
            (
                term("renderer"),
                Literal::new_simple_literal(renderer).into(),
            ),
            (
                term("destination"),
                Literal::new_simple_literal(destination).into(),
            ),
        ] {
            stream
                .write(Quad::new(
                    plan.clone(),
                    predicate,
                    object,
                    plan.clone(),
                ))
                .await?;
        }
        let placements = view.pattern(
            &format!("{profile} {} ?placement", term("placement")),
            0,
        );
        let order =
            view.pattern(&format!("?placement {} ?order", term("order")), 1);
        let mut placements = self.dataset.select(format!("SELECT DISTINCT ?placement ?order WHERE {{ {placements} {order} }} ORDER BY <http://www.w3.org/2001/XMLSchema#integer>(?order) ?placement")).await?;
        let selection = Selection {
            view:           &view,
            scope_snapshot: &scope_snapshot,
            scope_plan:     &scope_plan,
            plan:           &plan,
            profile:        &profile,
        };
        let mut previous = None;
        while let Some(row) = placements.next().await? {
            let Term::NamedNode(placement) = &row["placement"] else {
                bail!("placement must have a stable IRI")
            };
            let order = integer(&row["order"], "placement order")?;
            ensure!(
                previous != Some(order),
                "profile {profile} has ambiguous placement order {order}"
            );
            previous = Some(order);
            let component = view.node(placement, "component").await?;
            self.entries(
                &selection,
                placement,
                &component,
                order,
                &mut stream,
            )
            .await?;
        }
        self.assets(&selection, renderer_kind, &mut stream).await?;
        let receipt = stream.commit().await?;
        let snapshot = self.dataset.pin(&receipt).await?;
        Ok(ResolvedPlan { id: plan, snapshot })
    }

    async fn assets(
        &self,
        selection: &Selection<'_>,
        renderer: super::Renderer,
        stream: &mut ContributionStream,
    ) -> Result<()> {
        let members = Dataset::logical_pattern(
            selection.scope_snapshot,
            &format!(
                "{} {} ?asset",
                selection.scope_plan,
                term("member")
            ),
            0,
        );
        let scoped_subjects = Dataset::logical_pattern(
            selection.scope_snapshot,
            &format!(
                "{} {} ?selected",
                selection.scope_plan,
                term("member")
            ),
            2,
        );
        let direct = selection
            .view
            .pattern(&format!("?selected {} ?asset", term("asset")), 3);
        let fragment = selection.view.pattern(
            &format!("?fragment {} ?selected", term("subject")),
            4,
        );
        let attached = selection
            .view
            .pattern(&format!("?fragment {} ?asset", term("asset")), 5);
        let derived = selection.view.pattern(
            &format!("?fragment {} ?owner", term("derivedFrom")),
            6,
        );
        let inherited = selection
            .view
            .pattern(&format!("?owner {} ?asset", term("asset")), 7);
        let candidates = format!(
            "{{ {members} }} UNION {{ {scoped_subjects} {{ {direct} }} UNION \
             {{ {fragment} {{ {attached} }} UNION {{ {derived} {inherited} }} \
             }} }}"
        );
        let types = selection
            .view
            .pattern(&format!("?asset a {}", term("BinaryAsset")), 1);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?asset WHERE {{ {{ {candidates} }} {types} \
                 }} ORDER BY ?asset"
            ))
            .await?;
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(asset) = &row["asset"] else {
                bail!("plan asset must be an IRI")
            };
            if renderer != super::Renderer::Website {
                let mime = selection.view.text(asset, "format").await?;
                ensure!(
                    matches!(
                        mime.as_str(),
                        "image/png" | "image/jpeg" | "image/svg+xml"
                    ),
                    "non-image binary artifacts require the website renderer"
                );
            }
            count += 1;
            ensure!(count <= 256, "plan exceeds 256-asset budget");
            stream
                .write(Quad::new(
                    selection.plan.clone(),
                    term("asset"),
                    asset.clone(),
                    selection.plan.clone(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn entries(
        &self,
        selection: &Selection<'_>,
        placement: &NamedNode,
        component: &NamedNode,
        order: u32,
        stream: &mut ContributionStream,
    ) -> Result<()> {
        let view = selection.view;
        let scope_snapshot = selection.scope_snapshot;
        let scope_plan = selection.scope_plan;
        let plan = selection.plan;
        let fragments = view.pattern(
            &format!("{component} {} ?fragment", term("fragment")),
            0,
        );
        let subjects = view.pattern(
            &format!("?fragment {} ?subject", term("subject")),
            1,
        );
        let members = Dataset::logical_pattern(
            scope_snapshot,
            &format!("{scope_plan} {} ?subject", term("member")),
            2,
        );
        let fragment_order = view.pattern(
            &format!("?fragment {} ?fragmentOrder", term("order")),
            3,
        );
        let mut rows = self.dataset.select(format!("SELECT DISTINCT ?fragment ?fragmentOrder WHERE {{ {fragments} {subjects} {members} OPTIONAL {{ {fragment_order} }} }} ORDER BY <http://www.w3.org/2001/XMLSchema#integer>(?fragmentOrder) ?fragment")).await?;
        let mut count = 0u64;
        let mut previous = None;
        let mut missing_order = false;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(fragment) = &row["fragment"] else {
                bail!("fragment must have a stable IRI")
            };
            let fragment_order = row
                .get("fragmentOrder")
                .map(|value| integer(value, "fragment order"))
                .transpose()?;
            ensure!(
                count == 0 || (!missing_order && fragment_order.is_some()),
                "multiple selected fragments in {component} require explicit \
                 orders"
            );
            ensure!(
                fragment_order.is_none() || previous != fragment_order,
                "ambiguous fragment order in {component}"
            );
            missing_order |= fragment_order.is_none();
            previous = fragment_order;
            count += 1;
            self.references(selection, fragment).await?;
            let entry = identity("plan-entry", &[
                plan.as_str(),
                placement.as_str(),
                fragment.as_str(),
            ]);
            stream
                .write(Quad::new(
                    plan.clone(),
                    term("entry"),
                    entry.clone(),
                    plan.clone(),
                ))
                .await?;
            for (predicate, object) in [
                (
                    NamedNode::new_unchecked(RDF_TYPE),
                    Term::NamedNode(term("Entry")),
                ),
                (term("placement"), placement.clone().into()),
                (term("component"), component.clone().into()),
                (term("fragment"), fragment.clone().into()),
                (term("order"), Literal::from(order).into()),
                (
                    term("fragmentOrder"),
                    Literal::from(fragment_order.unwrap_or(0)).into(),
                ),
            ] {
                stream
                    .write(Quad::new(
                        entry.clone(),
                        predicate,
                        object,
                        plan.clone(),
                    ))
                    .await?;
            }
        }
        Ok(())
    }

    async fn references(
        &self,
        selection: &Selection<'_>,
        fragment: &NamedNode,
    ) -> Result<()> {
        let view = selection.view;
        let profile = selection.profile;
        let direct = view.pattern(
            &format!("{fragment} {} ?value", term("reference")),
            0,
        );
        let source = view.pattern(
            &format!("{fragment} {} ?subject", term("subject")),
            1,
        );
        let indirect = view.pattern(
            &format!("?subject {} ?value", term("reference")),
            2,
        );
        let mut references = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?value WHERE {{ {{ {direct} }} UNION {{ \
                 {source} {indirect} }} }}"
            ))
            .await?;
        while let Some(row) = references.next().await? {
            let Term::NamedNode(reference) = &row["value"] else {
                bail!("fragment reference must be an IRI")
            };
            if view.has_type(reference, "ExternalReference").await? {
                continue;
            }
            let placements = view.pattern(
                &format!("{profile} {} ?placement", term("placement")),
                0,
            );
            let component = view.pattern(
                &format!("?placement {} ?component", term("component")),
                1,
            );
            let fragments = view.pattern(
                &format!("?component {} ?fragment", term("fragment")),
                2,
            );
            let subject = view.pattern(
                &format!("?fragment {} {reference}", term("subject")),
                3,
            );
            let member = Dataset::logical_pattern(
                selection.scope_snapshot,
                &format!(
                    "{} {} {reference}",
                    selection.scope_plan,
                    term("member")
                ),
                4,
            );
            let mut resolved = self
                .dataset
                .select(format!(
                    "SELECT ?fragment WHERE {{ {placements} {component} \
                     {fragments} {subject} {member} }} LIMIT 1"
                ))
                .await?;
            ensure!(
                resolved.next().await?.is_some(),
                "{fragment} references missing content {reference}; declare \
                 external references explicitly"
            );
        }
        Ok(())
    }

    async fn check_destinations(
        &self,
        view: &RevisionView,
        project: &NamedNode,
    ) -> Result<()> {
        let mut targets = view.objects(project, "target").await?;
        while let Some(row) = targets.next().await? {
            let Term::NamedNode(target) = &row["value"] else {
                bail!("target must be an IRI")
            };
            let destination =
                destination(&view.text(target, "destination").await?)?;
            let mut peers = view.objects(project, "target").await?;
            while let Some(peer) = peers.next().await? {
                let Term::NamedNode(peer) = &peer["value"] else {
                    bail!("target must be an IRI")
                };
                if peer == target {
                    continue;
                }
                let other = destination_path(view, peer).await?;
                ensure!(
                    !destination.starts_with(&other) &&
                        !other.starts_with(&destination),
                    "targets {target} and {peer} have overlapping output \
                     destinations; explicit composition is required"
                );
            }
        }
        Ok(())
    }
}
