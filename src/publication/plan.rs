//! Resolve target-specific placements from reusable components and scoped
//! content.

mod books;
mod constraints;
mod requirements;
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
    std::collections::{HashMap, HashSet},
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
    orders:         HashMap<NamedNode, u32>,
    documented:     HashSet<NamedNode>,
    external:       HashSet<NamedNode>,
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
        let mut requirements =
            match requirements::Requirements::read(&view, &profile, target)
                .await
            {
                Ok(requirements) => requirements,
                Err(error) => {
                    return Err(requirements::failure(
                        writer,
                        &project,
                        target,
                        &self.revision,
                        error,
                    )
                    .await?)
                },
            };
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
                "urn:grapharch:target-planner:v2",
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
            orders:         self.fragment_orders(&view).await?,
            documented:     self
                .documented_subjects(
                    &view,
                    &profile,
                    &scope_snapshot,
                    &scope_plan,
                )
                .await?,
            external:       self.external_references(&view).await?,
        };
        let mut book_placements = Vec::new();
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
            let first = self
                .entries(
                    &selection,
                    placement,
                    &component,
                    order,
                    &mut stream,
                    &mut requirements,
                )
                .await?;
            if renderer_kind == super::Renderer::Bookshelf &&
                let Some(fragment) = first
            {
                book_placements.push((
                    placement.clone(),
                    component.clone(),
                    fragment,
                ));
            }
        }
        if let Err(error) = requirements.finish() {
            stream.abort().await?;
            return Err(requirements::failure(
                writer,
                &project,
                target,
                &self.revision,
                error,
            )
            .await?);
        }
        if renderer_kind == super::Renderer::Bookshelf {
            books::append(
                &selection,
                &book_placements,
                &self.dataset,
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

    async fn documented_subjects(
        &self,
        view: &RevisionView,
        profile: &NamedNode,
        scope_snapshot: &NamedNode,
        scope_plan: &NamedNode,
    ) -> Result<HashSet<NamedNode>> {
        let placements = view.pattern(
            &format!("{profile} {} ?placement", term("placement")),
            0,
        );
        let components = view.pattern(
            &format!("?placement {} ?component", term("component")),
            1,
        );
        let fragments = view.pattern(
            &format!("?component {} ?fragment", term("fragment")),
            2,
        );
        let subjects = view.pattern(
            &format!("?fragment {} ?subject", term("subject")),
            3,
        );
        let scope = Dataset::logical_pattern(
            scope_snapshot,
            &format!("{scope_plan} {} ?subject", term("member")),
            4,
        );
        self.named_index(format!(
            "SELECT DISTINCT ?subject WHERE {{ {placements} {components} \
             {fragments} {subjects} {scope} }}"
        ))
        .await
    }

    async fn external_references(
        &self,
        view: &RevisionView,
    ) -> Result<HashSet<NamedNode>> {
        let pattern = view.pattern(
            &format!("?subject a {}", term("ExternalReference")),
            0,
        );
        self.named_index(format!(
            "SELECT DISTINCT ?subject WHERE {{ {pattern} }}"
        ))
        .await
    }

    async fn named_index(&self, query: String) -> Result<HashSet<NamedNode>> {
        let mut rows = self.dataset.select(query).await?;
        let mut result = HashSet::new();
        while let Some(row) = rows.next().await? {
            ensure!(
                result.len() < super::limits::LINK_INDEX_ENTRIES,
                "publication reference index exceeds entry budget"
            );
            let Term::NamedNode(subject) = &row["subject"] else {
                bail!("publication subject must be a named identity")
            };
            result.insert(subject.clone());
        }
        Ok(result)
    }

    async fn fragment_orders(
        &self,
        view: &RevisionView,
    ) -> Result<HashMap<NamedNode, u32>> {
        let fragments = view.pattern(
            &format!("?fragment <{RDF_TYPE}> {}", term("Fragment")),
            0,
        );
        let order =
            view.pattern(&format!("?fragment {} ?value", term("order")), 1);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?fragment ?value WHERE {{ {fragments} \
                 {order} }}"
            ))
            .await?;
        let mut result = HashMap::new();
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(fragment) = &row["fragment"] else {
                bail!("fragment must have a stable IRI")
            };
            ensure!(
                result.len() < super::limits::PUBLICATION_ENTRIES as usize,
                "fragment order index exceeds publication entry budget"
            );
            let value = integer(&row["value"], "fragment order")?;
            if let Some(previous) = result.insert(fragment.clone(), value) {
                ensure!(
                    previous == value,
                    "conflicting fragment order for {fragment}"
                );
            }
        }
        Ok(result)
    }

    async fn entries(
        &self,
        selection: &Selection<'_>,
        placement: &NamedNode,
        component: &NamedNode,
        order: u32,
        stream: &mut ContributionStream,
        requirements: &mut requirements::Requirements,
    ) -> Result<Option<NamedNode>> {
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
        // The OPTIONAL order join used to re-evaluate the logical contribution
        // union for every fragment. Resolve this small metadata index once and
        // sort only this component's bounded fragment IDs, never its bodies.
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?fragment WHERE {{ {fragments} {subjects} \
                 {members} }}"
            ))
            .await?;
        let mut selected = Vec::new();
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(fragment) = &row["fragment"] else {
                bail!("fragment must have a stable IRI")
            };
            ensure!(
                selected.len() < super::limits::PUBLICATION_ENTRIES as usize,
                "component exceeds publication entry budget"
            );
            selected.push((
                selection.orders.get(fragment).copied(),
                fragment.clone(),
            ));
        }
        selected.sort();
        let role = requirements.role(view, placement).await?;
        let mut previous = None;
        let mut missing_order = false;
        for (count, (fragment_order, fragment)) in selected.iter().enumerate() {
            let fragment_order = *fragment_order;
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
            requirements
                .selected(view, role.as_deref(), fragment)
                .await?;
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
        Ok(selected.first().map(|(_, fragment)| fragment.clone()))
    }

    async fn references(
        &self,
        selection: &Selection<'_>,
        fragment: &NamedNode,
    ) -> Result<()> {
        let view = selection.view;
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
            ensure!(
                selection.external.contains(reference) ||
                    selection.documented.contains(reference),
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
