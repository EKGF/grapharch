//! Apply source labels only after the bounded diagram neighborhood is selected.
use {
    super::{
        Diagram,
        labels::{Label, disambiguate},
    },
    crate::publication::view::RevisionView,
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    std::collections::BTreeMap,
};

pub(super) async fn apply(
    diagram: &mut Diagram,
    view: &RevisionView,
    language: &str,
) -> Result<()> {
    let mut unique = BTreeMap::new(); // bounded: admitted diagram endpoints and shape fields (357).
    for node in diagram.nodes() {
        if matches!(
            node.kind.as_str(),
            "OWL expression" | "Literal value"
        ) {
            continue;
        }
        unique
            .entry(node.id.clone())
            .or_insert_with(|| node.label.clone());
    }
    ensure!(
        unique.len() <= 357,
        "diagram label resource budget exceeded"
    );
    let mut labels = Vec::new(); // bounded: 357 admitted distinct resources.
    for (id, qname) in unique {
        labels.push(
            Label::read(view, &NamedNode::new(id)?, qname, language).await?,
        );
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
    }
    Ok(())
}
