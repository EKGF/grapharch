//! Readable taxonomy overview derived from the same pinned model as detail
//! pages.
use {
    super::{
        MaturityModelDocumentor,
        escape,
        link,
        model::{self, Kind, Model},
        pages::{Page, valid_segment},
    },
    crate::publication::dataset::identity,
    anyhow::{Result, ensure},
};
impl MaturityModelDocumentor {
    pub(crate) async fn overview_page(
        &self,
        route: &str,
    ) -> Result<(Page, String)> {
        ensure!(
            !route.is_empty() &&
                route.ends_with('/') &&
                route.trim_end_matches('/').split('/').all(valid_segment),
            "maturity overview route must be a relative directory"
        );
        let model = Model::read(&self.dataset, &self.view()).await?;
        let subject = identity("maturity-overview", &[self.project.as_str()]);
        let mut body = String::from(
            "| Pillar | Capability area | Number | Capability | Measurable \
             ability |\n|---|---|---|---|---|\n",
        );
        for id in model.order() {
            let node = &model.nodes[&id];
            if node.kind != Kind::Capability {
                continue;
            }
            let area = node.parent.as_ref().unwrap();
            let pillar = model.nodes[area].parent.as_ref().unwrap();
            let comment =
                model::text(&self.view(), &id, model::COMMENT, false).await?;
            body.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                link(&model, pillar),
                link(&model, area),
                escape(&node.number),
                link(&model, &id),
                escape(&comment).replace('\n', " ")
            ));
            ensure!(
                body.len() <= 512 * 1024,
                "maturity overview exceeds 512 KiB"
            );
        }
        Ok((
            Page {
                summary: self.fragment_id(&subject),
                subject,
                title: "Overview".into(),
                route: route.into(),
                parent: None,
                companion_directory: None,
                order: 0,
            },
            body,
        ))
    }
}
