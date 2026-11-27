//! The organization identity of a website header: a leading brand logo
//! (`ga:brandLogo`) and further identity marks (`ga:identityMark`) at its end.
//! The profile declares them, or else the nearest theme of its lineage, so a
//! house theme carries its organization's identity to every site using it.
use {
    super::{brand_mark::BrandMark, chrome_sources::ChromeSources},
    crate::publication::render::{model::Plan, website::site_base::SiteBases},
    anyhow::{Result, ensure},
    std::path::Path,
};

const MAX_MARKS: usize = 4;

pub(in crate::publication::render::website) struct SiteIdentity {
    brand: Option<BrandMark>,
    // bounded: at most MAX_MARKS identity marks.
    marks: Vec<BrandMark>,
}

impl SiteIdentity {
    pub async fn read(plan: &Plan, sources: &ChromeSources) -> Result<Self> {
        let bases = SiteBases::read(plan).await?;
        let brand = match sources.nearest(plan, "brandLogo").await? {
            None => None,
            Some(subject) => {
                let mut rows =
                    plan.source.objects(subject, "brandLogo").await?;
                let row = rows.next().await?.expect("declared brand logo");
                ensure!(
                    rows.next().await?.is_none(),
                    "{subject} declares more than one ga:brandLogo"
                );
                Some(BrandMark::read(plan, &row["value"], &bases).await?)
            },
        };
        let mut marks = Vec::new();
        if let Some(subject) = sources.nearest(plan, "identityMark").await? {
            let mut rows = plan.source.objects(subject, "identityMark").await?;
            while let Some(row) = rows.next().await? {
                ensure!(
                    marks.len() < MAX_MARKS,
                    "{subject} declares more than {MAX_MARKS} identity marks"
                );
                marks.push(BrandMark::read(plan, &row["value"], &bases).await?);
            }
        }
        marks.sort_by_key(BrandMark::order);
        ensure!(
            marks
                .windows(2)
                .all(|pair| pair[0].order() != pair[1].order()),
            "identity marks require distinct orders"
        );
        Ok(Self { brand, marks })
    }

    /// Whether the header carries any organization identity.
    pub fn is_empty(&self) -> bool {
        self.brand.is_none() && self.marks.is_empty()
    }

    /// The organization the header navigation belongs to, for its label.
    pub fn organization(&self) -> Option<&str> {
        self.brand.as_ref().map(BrandMark::title)
    }

    pub fn brand_html(&self, prefix: &str) -> String {
        self.brand
            .as_ref()
            .map(|brand| brand.brand_html(prefix))
            .unwrap_or_default()
    }

    pub fn marks_html(&self, prefix: &str) -> String {
        self.marks
            .iter()
            .map(|mark| mark.mark_html(prefix))
            .collect()
    }

    /// Publish every embedded image once.
    pub async fn assets(
        &self,
        stage: &Path,
        files: &mut Vec<String>,
    ) -> Result<()> {
        for mark in self.brand.iter().chain(&self.marks) {
            let Some((route, bytes)) = mark.image().embedded() else {
                continue;
            };
            if files.iter().any(|file| file == route) {
                continue;
            }
            let path = stage.join(route);
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(path, bytes).await?;
            files.push(route.to_owned());
        }
        Ok(())
    }
}
