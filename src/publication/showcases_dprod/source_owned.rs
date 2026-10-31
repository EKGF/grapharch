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
            view.text(target, "renderer").await?.as_str(),
            "specification" | "typst"
        ) {
            continue;
        }
        let profile = view.node(target, "profile").await?;
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
