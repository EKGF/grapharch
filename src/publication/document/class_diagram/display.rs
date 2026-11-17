//! Apply source labels only after the bounded diagram neighborhood is selected.
//! Every box, edge and field follows one rule: a resource sharing the focus
//! term's ontology shows its label, one from another ontology shows
//! "label (prefix:Local)", and an unlabelled one shows its prefixed name.
use {
    super::{
        Diagram,
        NodeKind,
        labels::{Label, LabelRole, disambiguate},
        ontology::OntologyMembership,
    },
    crate::publication::view::RevisionView,
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    std::collections::{BTreeMap, BTreeSet},
};

pub(super) async fn apply(
    diagram: &mut Diagram,
    view: &RevisionView,
    language: &str,
) -> Result<()> {
    let mut unique = BTreeMap::new(); // bounded: admitted diagram endpoints and shape fields (357).
    for node in diagram.nodes() {
        if matches!(
            node.kind,
            NodeKind::OwlExpression |
                NodeKind::OwlUnion |
                NodeKind::LiteralValue
        ) {
            continue;
        }
        unique.entry(node.id.clone()).or_insert_with(|| {
            let role = if node.kind.is_property() {
                LabelRole::Property
            } else {
                LabelRole::Box
            };
            (node.label.clone(), role)
        });
    }
    ensure!(
        unique.len() <= 357,
        "diagram label resource budget exceeded"
    );
    let mut labels = Vec::new(); // bounded: 357 admitted distinct resources.
    let focus = OntologyMembership::read(
        view,
        &NamedNode::new(diagram.focus.id.clone())?,
    )
    .await?;
    let mut imported = BTreeSet::new(); // bounded: 357 admitted distinct resources.
    for (id, (qname, role)) in unique {
        let id = NamedNode::new(id)?;
        let mut label = Label::read(view, &id, qname, role, language).await?;
        if !OntologyMembership::read(view, &id)
            .await?
            .shares_ontology_with(&focus)
        {
            label.qualify();
            imported.insert(id.into_string());
        }
        labels.push(label);
    }
    disambiguate(&mut labels)?;
    let labels: BTreeMap<_, _> = labels
        .into_iter()
        .map(|label| (label.iri, label.text))
        .collect(); // bounded: 357 resources.
    for node in diagram.nodes_mut() {
        if let Some(label) = labels.get(&node.id) {
            node.label.clone_from(label);
        }
        node.imported = imported.contains(&node.id);
    }
    Ok(())
}
