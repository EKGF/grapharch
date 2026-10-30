use {
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, HashSet},
};
pub(super) const UC: &str = "https://ekgf.org/ontology/use-case#";
pub(super) struct Related {
    pub predicate:   String,
    pub subject:     NamedNode,
    pub label:       String,
    pub description: String,
}
pub(super) struct Node {
    pub label:   String,
    pub parent:  Option<NamedNode>,
    pub used_in: Vec<NamedNode>,
}
pub(super) struct Tree {
    // bounded: 4096 identities, 65536 edges, and 16 MiB of topology metadata.
    // Descriptions and fragment bodies are read one use case at a time.
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
            "use-case annotation budget exceeded: {id}"
        );
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!("use-case text must be literal: {id}")
        };
        if super::super::description::selected(value, "en") {
            ensure!(
                selected.is_none(),
                "ambiguous English use-case annotation: {id} {predicate}"
            );
            ensure!(
                value.value().len() <= 256 * 1024,
                "use-case text exceeds byte budget"
            );
            selected = Some(value.value().to_owned());
        }
    }
    ensure!(
        !required || selected.as_ref().is_some_and(|s| !s.trim().is_empty()),
        "missing English use-case annotation: {id} {predicate}"
    );
    Ok(selected.unwrap_or_default())
}
fn english(variable: &str) -> String {
    format!(
        "FILTER(isLiteral({variable}) && (LANG({variable}) = '' || \
         LCASE(LANG({variable})) = 'en'))"
    )
}
/// Batch related labels and descriptions, never issue a query per graph edge.
pub(super) async fn related(
    dataset: &Dataset,
    view: &RevisionView,
    id: &NamedNode,
) -> Result<Vec<Related>> {
    let facts = view.pattern(&format!("{id} ?predicate ?target"), 0);
    let labels = view.pattern(
        "?target <http://www.w3.org/2000/01/rdf-schema#label> ?label",
        1,
    );
    let descriptions = view.pattern(
        "?target <http://www.w3.org/2000/01/rdf-schema#comment> ?description",
        2,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?predicate ?target ?label ?description WHERE {{ \
             {facts} VALUES ?predicate {{ <{UC}hasStereoType> \
             <{UC}involvesPersona> <{UC}hasOutcome> <{UC}usesConcept> }} \
             OPTIONAL {{ {labels} {} }} OPTIONAL {{ {descriptions} {} }} }} \
             ORDER BY ?predicate ?target LIMIT 513",
            english("?label"),
            english("?description")
        ))
        .await?;
    let mut result = Vec::new(); // bounded: 64 related resources and 256 KiB combined text/identity bytes.
    let mut bytes = 0;
    while let Some(row) = rows.next().await? {
        ensure!(
            result.len() < 64,
            "use-case related-resource budget exceeded"
        );
        let (Term::NamedNode(predicate), Term::NamedNode(target)) =
            (&row["predicate"], &row["target"])
        else {
            anyhow::bail!("use-case relation requires an IRI: {id}")
        };
        ensure!(
            !result.iter().any(|r: &Related| {
                r.predicate == predicate.as_str().trim_start_matches(UC) &&
                    &r.subject == target
            }),
            "ambiguous related-resource labels or descriptions: {target}"
        );
        let Some(Term::Literal(label)) = row.get("label") else {
            anyhow::bail!("related resource lacks an English label: {target}")
        };
        ensure!(
            !label.value().trim().is_empty() && label.value().len() <= 160,
            "invalid related-resource label: {target}"
        );
        let description = match row.get("description") {
            None => String::new(),
            Some(Term::Literal(v)) => v.value().to_owned(),
            _ => anyhow::bail!("related-resource description must be text"),
        };
        bytes +=
            label.value().len() + description.len() + target.as_str().len();
        ensure!(
            bytes <= 256 * 1024,
            "use-case related-resource text budget exceeded"
        );
        result.push(Related {
            predicate: predicate.as_str().strip_prefix(UC).unwrap().into(),
            subject: target.clone(),
            label: label.value().into(),
            description,
        });
    }
    Ok(result)
}
impl Tree {
    pub async fn read(dataset: &Dataset, view: &RevisionView) -> Result<Self> {
        let typed = view.pattern(&format!("?subject a <{UC}UseCase>"), 0);
        let labels = view.pattern(
            "?subject <http://www.w3.org/2000/01/rdf-schema#label> ?label",
            1,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?subject ?label WHERE {{ {typed} OPTIONAL {{ \
                 {labels} {} }} }} ORDER BY ?subject LIMIT 8193",
                english("?label")
            ))
            .await?;
        let mut nodes = BTreeMap::new();
        let mut bytes = 0;
        while let Some(row) = rows.next().await? {
            ensure!(
                nodes.len() < 4096,
                "use-case tree exceeds 4096-node budget"
            );
            let Term::NamedNode(id) = &row["subject"] else {
                anyhow::bail!("use cases require named identities")
            };
            let Some(Term::Literal(label)) = row.get("label") else {
                anyhow::bail!("missing English use-case label: {id}")
            };
            ensure!(
                !label.value().trim().is_empty() && label.value().len() <= 160,
                "invalid use-case label: {id}"
            );
            ensure!(
                !nodes.contains_key(id),
                "ambiguous English use-case label: {id}"
            );
            bytes += id.as_str().len() + label.value().len();
            ensure!(
                bytes <= 16 * 1024 * 1024,
                "use-case topology exceeds 16 MiB"
            );
            nodes.insert(id.clone(), Node {
                label:   label.value().into(),
                parent:  None,
                used_in: Vec::new(),
            });
        }
        let facts = view.pattern("?subject ?predicate ?target", 2);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?subject ?predicate ?target WHERE {{ {typed} \
                 {facts} VALUES ?predicate {{ <{UC}isPartOf> <{UC}isUsedIn> \
                 }} }} ORDER BY ?subject ?predicate ?target LIMIT 65537"
            ))
            .await?;
        let mut edges = 0;
        while let Some(row) = rows.next().await? {
            edges += 1;
            ensure!(
                edges <= 65536,
                "use-case topology exceeds 65536 edges"
            );
            let (
                Term::NamedNode(id),
                Term::NamedNode(predicate),
                Term::NamedNode(target),
            ) = (&row["subject"], &row["predicate"], &row["target"])
            else {
                anyhow::bail!("use-case topology requires named endpoints")
            };
            ensure!(
                id != target,
                "self-referential use-case relation: {id}"
            );
            bytes += target.as_str().len();
            ensure!(
                bytes <= 16 * 1024 * 1024,
                "use-case topology exceeds 16 MiB"
            );
            let node = nodes
                .get_mut(id)
                .ok_or_else(|| anyhow::anyhow!("unknown use case: {id}"))?;
            if predicate.as_str() == format!("{UC}isPartOf") {
                ensure!(
                    node.parent.is_none(),
                    "multiple ownership parents: {id}"
                );
                node.parent = Some(target.clone());
            } else {
                ensure!(
                    node.used_in.len() < 64,
                    "use-case reuse budget exceeded"
                );
                node.used_in.push(target.clone());
            }
        }
        let tree = Self { nodes };
        tree.validate()?;
        Ok(tree)
    }

    fn validate(&self) -> Result<()> {
        for (id, node) in &self.nodes {
            for target in node.parent.iter().chain(&node.used_in) {
                ensure!(
                    self.nodes.contains_key(target),
                    "dangling use-case relation: {id} -> {target}"
                );
            }
        }
        for id in self.nodes.keys() {
            let mut seen = HashSet::new(); // bounded: one ownership chain, at most 32 levels.
            let mut current = Some(id);
            while let Some(id) = current {
                ensure!(
                    seen.insert(id),
                    "use-case ownership cycle at {id}"
                );
                ensure!(
                    seen.len() <= 32,
                    "use-case ownership depth exceeds 32"
                );
                current = self.nodes[id].parent.as_ref();
            }
        }
        Ok(())
    }

    pub fn sorted(&self, mut ids: Vec<NamedNode>) -> Vec<NamedNode> {
        ids.sort_by(|a, b| {
            self.nodes[a].label.cmp(&self.nodes[b].label).then(a.cmp(b))
        });
        ids
    }

    pub fn children(&self, id: &NamedNode, reuse: bool) -> Vec<NamedNode> {
        self.sorted(
            self.nodes
                .iter()
                .filter(|(_, n)| {
                    n.parent.as_ref() == Some(id) ||
                        (reuse && n.used_in.contains(id))
                })
                .map(|(id, _)| id.clone())
                .collect(),
        )
    }

    pub fn order(&self) -> Vec<NamedNode> {
        fn visit(tree: &Tree, id: NamedNode, result: &mut Vec<NamedNode>) {
            result.push(id.clone());
            for child in tree.children(&id, false) {
                visit(tree, child, result);
            }
        }
        let mut order = Vec::new(); // bounded: exactly the validated nodes, at most 4096.
        for root in self.sorted(
            self.nodes
                .iter()
                .filter(|(_, n)| n.parent.is_none())
                .map(|(id, _)| id.clone())
                .collect(),
        ) {
            visit(self, root, &mut order);
        }
        order
    }
}
