//! Stable website placement descriptors derived from maturity-model semantics.
use {
    super::{
        MaturityModelDocumentor,
        model::{Kind, MM, Model, text},
    },
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    std::collections::{BTreeMap, BTreeSet},
};

pub(crate) struct Page {
    pub subject:             NamedNode,
    pub title:               String,
    pub route:               String,
    pub parent:              Option<NamedNode>,
    pub summary:             NamedNode,
    pub companion_directory: Option<String>,
    pub order:               u32,
}
impl MaturityModelDocumentor {
    pub(crate) async fn website_pages(&self, root: &str) -> Result<Vec<Page>> {
        ensure!(
            !root.is_empty() &&
                root.ends_with('/') &&
                root.trim_end_matches('/').split('/').all(valid_segment),
            "maturity route must be a nonempty relative directory"
        );
        let view = self.view();
        let model = Model::read(&self.dataset, &view).await?;
        let mut routes = BTreeMap::new(); // bounded: validated model's 4096-node limit.
        let mut unique = BTreeSet::new();
        let mut pages = Vec::new();
        for subject in model.order() {
            let node = &model.nodes[&subject];
            let local = text(
                &view,
                &subject,
                &format!("{MM}iriLocalName"),
                true,
            )
            .await?;
            ensure!(
                valid_segment(&local),
                "invalid maturity iriLocalName: {subject}"
            );
            let route = match node.kind {
                Kind::Model => root.to_string(),
                Kind::Level => format!("{root}level/{local}/"),
                _ => {
                    format!(
                        "{}{local}/",
                        routes.get(node.parent.as_ref().unwrap()).ok_or_else(
                            || anyhow::anyhow!("maturity parent route missing")
                        )?
                    )
                },
            };
            ensure!(
                unique.insert(route.clone()),
                "duplicate maturity route: {route}"
            );
            routes.insert(subject.clone(), route.clone());
            pages.push(Page {
                subject:             subject.clone(),
                title:               node.label.clone(),
                route:               route.clone(),
                parent:              node.parent.clone(),
                summary:             self.fragment_id(&subject),
                companion_directory: match node.kind {
                    Kind::Model | Kind::Level => None,
                    _ => {
                        Some(format!(
                            "pillar/{}",
                            route.strip_prefix(root).unwrap()
                        ))
                    },
                },
                order:               pages.len() as u32,
            });
        }
        ensure!(
            pages.len() <= 256,
            "native maturity website exceeds 256-page budget"
        );
        Ok(pages)
    }
}
pub(super) fn valid_segment(value: &str) -> bool {
    !value.is_empty() &&
        value.len() <= 128 &&
        value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
        })
}
