//! Bind generated reference groups beneath explicitly authored reference roots.
use {
    super::{node, quad},
    crate::publication::{
        term,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
};

pub(super) struct ReferenceRoot {
    profile:   NamedNode,
    placement: NamedNode,
    order:     u32,
}

pub(super) async fn roots(view: &RevisionView) -> Result<Vec<ReferenceRoot>> {
    // bounded: a source may register at most 16 publication targets.
    let mut result = Vec::new();
    let mut targets = view.objects(&node("project"), "target").await?;
    let mut count = 0;
    while let Some(row) = targets.next().await? {
        count += 1;
        ensure!(count <= 16, "DPROD target budget exceeded");
        let Term::NamedNode(target) = &row["value"] else {
            anyhow::bail!("publication target must be an IRI")
        };
        if !matches!(
            view.renderer(target).await?.as_str(),
            "specification" | "typst"
        ) {
            continue;
        }
        let profile = view.node(target, "profile").await?;
        // Editorial overviews deliberately exclude the technical catalogue.
        if super::super::theme::Lineage::read(
            view,
            &profile,
            super::super::theme::Builtin::Specification,
        )
        .await?
        .refines(super::super::theme::Builtin::Overview)
        {
            continue;
        }
        if result
            .iter()
            .any(|root: &ReferenceRoot| root.profile == profile)
        {
            continue;
        }
        let mut placements = view.objects(&profile, "placement").await?;
        let mut root = None;
        let mut count = 0;
        while let Some(row) = placements.next().await? {
            count += 1;
            ensure!(count <= 1024, "DPROD placement budget exceeded");
            let Term::NamedNode(placement) = &row["value"] else {
                anyhow::bail!("placement must be an IRI")
            };
            if view.optional(placement, "sectionRole").await? !=
                Some(Literal::new_simple_literal("reference").into())
            {
                continue;
            }
            ensure!(
                root.is_none(),
                "profile has multiple reference roots"
            );
            let order = view.text(placement, "order").await?.parse::<u32>()?;
            root = Some(ReferenceRoot {
                profile: profile.clone(),
                placement: placement.clone(),
                order,
            });
        }
        result.push(root.ok_or_else(|| {
            anyhow::anyhow!(
                "{profile} needs a source-owned reference placement"
            )
        })?);
    }
    ensure!(
        !result.is_empty(),
        "no source-owned reference profiles"
    );
    Ok(result)
}

impl ReferenceRoot {
    pub(super) async fn place(
        &self,
        stream: &mut ContributionStream,
        key: &str,
        title: &str,
        offset: u32,
    ) -> Result<()> {
        let placement = NamedNode::new(format!(
            "{}:generated:{key}",
            self.profile.as_str()
        ))?;
        let component = node(&format!("component:{key}"));
        let metadata: [(NamedNode, &str, Term); 5] = [
            (
                self.profile.clone(),
                "placement",
                placement.clone().into(),
            ),
            (
                placement.clone(),
                "component",
                component.clone().into(),
            ),
            (
                placement.clone(),
                "parentPlacement",
                self.placement.clone().into(),
            ),
            (
                placement.clone(),
                "order",
                Literal::from(self.order.checked_add(offset).ok_or_else(
                    || anyhow::anyhow!("reference order overflow"),
                )?)
                .into(),
            ),
            (
                placement.clone(),
                "sectionKind",
                Literal::new_simple_literal("section").into(),
            ),
        ];
        for (subject, field, value) in metadata {
            stream.write(quad(&subject, field, value)).await?;
        }
        let triples: [(NamedNode, NamedNode, Term); 3] = [
            (
                placement,
                NamedNode::new_unchecked(crate::publication::RDF_TYPE),
                term("Placement").into(),
            ),
            (
                component.clone(),
                NamedNode::new_unchecked(crate::publication::RDF_TYPE),
                term("Component").into(),
            ),
            (
                component,
                NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                Literal::new_simple_literal(title).into(),
            ),
        ];
        for (subject, predicate, object) in triples {
            stream
                .write(Quad::new(
                    subject,
                    predicate,
                    object,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        Ok(())
    }
}

pub(super) async fn legal_targets(view: &RevisionView) -> Result<Vec<String>> {
    // bounded: at most 16 registered targets.
    let mut result = Vec::new();
    let mut rows = view.objects(&node("project"), "target").await?;
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        count += 1;
        ensure!(count <= 16, "DPROD target budget exceeded");
        let Term::NamedNode(target) = &row["value"] else {
            anyhow::bail!("target must be an IRI")
        };
        if matches!(
            view.renderer(target).await?.as_str(),
            "specification" | "typst"
        ) {
            let profile = view.node(target, "profile").await?;
            if !super::super::theme::Lineage::read(
                view,
                &profile,
                super::super::theme::Builtin::Specification,
            )
            .await?
            .refines(super::super::theme::Builtin::Overview)
            {
                result.push(target.as_str().to_owned());
            }
        }
    }
    ensure!(!result.is_empty(), "no legal publication targets");
    Ok(result)
}

/// Resolve the source's active publication targets, rather than selecting a
/// fixed number of showcase outputs. A future formal-release template need not
/// be registered as an active project target.
pub(super) async fn targets(view: &RevisionView) -> Result<Vec<NamedNode>> {
    // bounded: publication definitions may register at most16 targets.
    let mut targets = Vec::new();
    let mut rows = view.objects(&node("project"), "target").await?;
    while let Some(row) = rows.next().await? {
        ensure!(targets.len() < 16, "DPROD target budget exceeded");
        let Term::NamedNode(target) = &row["value"] else {
            anyhow::bail!("publication target must be an IRI")
        };
        targets.push(target.clone());
    }
    ensure!(
        !targets.is_empty(),
        "project has no active publication targets"
    );
    let primary = node("specification");
    targets.sort_by(|a, b| {
        (a != &primary, a.as_str()).cmp(&(b != &primary, b.as_str()))
    });
    Ok(targets)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn editorial_overview_does_not_require_a_reference_catalogue()
    -> anyhow::Result<()> {
        use crate::publication::{
            collect::{Collector, RdfFile},
            dataset::Dataset,
            view::RevisionView,
            writer::GraphWriter,
        };
        let source = tempfile::tempdir()?;
        let path = source.path().join("targets.ttl");
        tokio::fs::write(
            &path,
            r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix ex: <urn:dprod-showcase:> .
ex:project ga:target ex:spec, ex:overview .
ex:spec ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer>; ga:profile ex:spec-profile .
ex:spec-profile ga:placement ex:reference .
ex:reference ga:sectionRole "reference"; ga:order 1 .
ex:overview ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer>; ga:profile ex:overview-profile .
ex:overview-profile ga:theme ga:OverviewTheme .
"#,
        )
        .await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
        RdfFile::new(super::super::PROJECT, "urn:fixture", path)?
            .collect(&writer)
            .await?;
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for(super::super::PROJECT).await?,
        );
        let result = super::roots(&view).await;
        drop(writer);
        worker.await??;
        assert_eq!(result?.len(), 1);
        assert_eq!(super::legal_targets(&view).await?, vec![
            "urn:dprod-showcase:spec"
        ]);
        Ok(())
    }
}
