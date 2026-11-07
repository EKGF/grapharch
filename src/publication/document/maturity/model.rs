use {
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};
pub(super) const MM: &str = "https://raw.githubusercontent.com/EKGF/ontology-maturity-model/main/maturity-model.ttl#";
pub(super) const LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
pub(super) const COMMENT: &str = "http://www.w3.org/2000/01/rdf-schema#comment";
pub(super) const DESCRIPTION: &str = "http://purl.org/dc/terms/description";
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Model,
    Pillar,
    Area,
    Capability,
    Level,
}
impl Kind {
    pub fn local(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Pillar => "Pillar",
            Self::Area => "CapabilityArea",
            Self::Capability => "Capability",
            Self::Level => "Level",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Area => "Capability area",
            _ => self.local(),
        }
    }

    pub fn parent(self) -> Option<(&'static str, Self)> {
        match self {
            Self::Model => None,
            Self::Pillar => Some(("pillarInModel", Self::Model)),
            Self::Area => Some(("inPillar", Self::Pillar)),
            Self::Capability => Some(("inArea", Self::Area)),
            Self::Level => Some(("levelInModel", Self::Model)),
        }
    }
}
pub(super) struct Node {
    pub kind:   Kind,
    pub label:  String,
    pub number: String,
    pub parent: Option<NamedNode>,
    pub sort:   String,
}
pub(super) struct Model {
    // bounded: at most 4096 subjects and 8 MiB metadata; descriptions read
    // individually.
    pub nodes: BTreeMap<NamedNode, Node>,
}
pub(super) async fn text(
    view: &RevisionView,
    id: &NamedNode,
    predicate: &str,
    required: bool,
) -> Result<String> {
    let mut rows = view.objects_by(id, &NamedNode::new(predicate)?).await?;
    let mut selected = None;
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        count += 1;
        ensure!(
            count <= 64,
            "maturity annotation budget exceeded: {id}"
        );
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!("maturity text must be literal: {id} {predicate}")
        };
        if super::super::description::selected(value, "en") {
            ensure!(
                selected.is_none(),
                "ambiguous maturity annotation: {id} {predicate}"
            );
            ensure!(
                value.value().len() <= 256 * 1024,
                "maturity text exceeds byte budget: {id}"
            );
            selected = Some(value.value().to_owned());
        }
    }
    ensure!(
        !required || selected.as_ref().is_some_and(|v| !v.trim().is_empty()),
        "missing maturity annotation: {id} {predicate}"
    );
    Ok(selected.unwrap_or_default())
}
fn natural(value: &str) -> String {
    let mut output = String::new();
    let mut digits = String::new();
    for c in value.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        if !digits.is_empty() {
            output.push_str(&format!("{:0>20}", digits));
            digits.clear();
        }
        if c != '\0' {
            output.push(c.to_ascii_lowercase());
        }
    }
    output
}
impl Model {
    pub async fn read(dataset: &Dataset, view: &RevisionView) -> Result<Self> {
        let types = view.pattern("?subject a ?kind", 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?subject ?kind WHERE {{ {types} VALUES ?kind \
                 {{ <{MM}Model> <{MM}Pillar> <{MM}CapabilityArea> \
                 <{MM}Capability> <{MM}Level> }} }} ORDER BY ?subject ?kind \
                 LIMIT 4097"
            ))
            .await?;
        let mut nodes = BTreeMap::new();
        let mut bytes = 0;
        while let Some(row) = rows.next().await? {
            ensure!(
                nodes.len() < 4096,
                "maturity model exceeds 4096-subject budget"
            );
            let (Term::NamedNode(id), Term::NamedNode(class)) =
                (&row["subject"], &row["kind"])
            else {
                anyhow::bail!("maturity subjects require named identities")
            };
            ensure!(
                !nodes.contains_key(id),
                "maturity subject has conflicting kinds: {id}"
            );
            let kind = match class.as_str().strip_prefix(MM).unwrap() {
                "Model" => Kind::Model,
                "Pillar" => Kind::Pillar,
                "CapabilityArea" => Kind::Area,
                "Capability" => Kind::Capability,
                "Level" => Kind::Level,
                _ => unreachable!(),
            };
            let label = text(view, id, LABEL, true).await?;
            let number =
                text(view, id, &format!("{MM}capabilityNumber"), false).await?;
            let sort = text(view, id, &format!("{MM}sortKey"), false).await?;
            ensure!(
                label.len() <= 512 && number.len() <= 64 && sort.len() <= 512,
                "maturity metadata exceeds field budget: {id}"
            );
            let parent = if let Some((predicate, _)) = kind.parent() {
                match view
                    .optional_by(
                        id,
                        &NamedNode::new(format!("{MM}{predicate}"))?,
                    )
                    .await?
                {
                    Some(Term::NamedNode(parent)) => Some(parent),
                    _ => {
                        anyhow::bail!(
                            "missing or invalid maturity parent {predicate}: \
                             {id}"
                        )
                    },
                }
            } else {
                None
            };
            bytes += id.as_str().len() +
                label.len() +
                number.len() +
                sort.len() +
                parent.as_ref().map_or(0, |v| v.as_str().len());
            ensure!(
                bytes <= 8 * 1024 * 1024,
                "maturity metadata exceeds 8 MiB budget"
            );
            let sort = natural(
                if !sort.is_empty() {
                    &sort
                } else if !number.is_empty() {
                    &number
                } else {
                    &label
                },
            );
            nodes.insert(id.clone(), Node {
                kind,
                label,
                number,
                parent,
                sort,
            });
        }
        for (id, node) in &nodes {
            if let Some(parent) = &node.parent {
                ensure!(
                    nodes.get(parent).is_some_and(|v| {
                        v.kind == node.kind.parent().unwrap().1
                    }),
                    "dangling or incorrectly typed maturity parent: {id} -> \
                     {parent}"
                );
            }
        }
        Ok(Self { nodes })
    }

    pub fn children(&self, id: &NamedNode, kind: Kind) -> Vec<&NamedNode> {
        // bounded: validated model has at most 4096 nodes.
        let mut children: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.parent.as_ref() == Some(id) && n.kind == kind)
            .map(|(id, _)| id)
            .collect();
        children.sort_by(|a, b| {
            self.nodes[*a]
                .sort
                .cmp(&self.nodes[*b].sort)
                .then_with(|| a.cmp(b))
        });
        children
    }

    pub fn order(&self) -> Vec<NamedNode> {
        // bounded: validated model has at most 4096 nodes; depth is fixed by
        // kind.
        let mut result = Vec::with_capacity(self.nodes.len());
        for (id, node) in &self.nodes {
            if node.kind == Kind::Model {
                self.append(id, &mut result);
            }
        }
        result
    }

    fn append(&self, id: &NamedNode, result: &mut Vec<NamedNode>) {
        result.push(id.clone());
        for kind in [Kind::Pillar, Kind::Area, Kind::Capability, Kind::Level] {
            for child in self.children(id, kind) {
                self.append(child, result);
            }
        }
    }
}
