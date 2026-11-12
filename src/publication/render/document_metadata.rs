//! PDF document metadata derived from the pinned target definition.
use {
    super::{markup::quote, model::Plan, publication_date::PublicationDate},
    anyhow::{Result, bail, ensure},
    oxrdf::Term,
    std::collections::BTreeSet,
};

/// The upper bound on declared keywords for one target.
const MAX_KEYWORDS: usize = 64;

/// Title, authors, keywords and date embedded in the PDF information
/// dictionary and XMP packet. Nothing is derived from the build environment,
/// so the same pinned plan always yields the same metadata.
pub(super) struct DocumentMetadata {
    title:    String,
    authors:  Vec<String>,
    // bounded: at most MAX_KEYWORDS declared target keywords.
    keywords: BTreeSet<String>,
    date:     Option<PublicationDate>,
}

impl DocumentMetadata {
    /// Collect the metadata for one document of the plan's target. `authors`
    /// are the declared authors, or else the publishing authority.
    pub(super) async fn read(
        plan: &Plan,
        title: &str,
        authors: Vec<String>,
    ) -> Result<Self> {
        ensure!(
            !authors.is_empty(),
            "a PDF requires at least one author"
        );
        Ok(Self {
            title: title.to_owned(),
            authors,
            keywords: Self::keywords(plan).await?,
            date: PublicationDate::read(plan).await?,
        })
    }

    async fn keywords(plan: &Plan) -> Result<BTreeSet<String>> {
        let mut keywords = BTreeSet::new();
        let mut rows = plan.source.objects(&plan.target, "keyword").await?;
        while let Some(row) = rows.next().await? {
            let Term::Literal(keyword) = &row["value"] else {
                bail!("{} keyword must be a string literal", plan.target)
            };
            ensure!(
                keyword.datatype().as_str() ==
                    "http://www.w3.org/2001/XMLSchema#string",
                "{} keyword must be an xsd:string",
                plan.target
            );
            let value = keyword.value();
            ensure!(
                !value.trim().is_empty() && value.trim() == value,
                "{} keyword must be non-empty without surrounding space",
                plan.target
            );
            ensure!(
                !value.contains([',', ';']),
                "{} keyword {value:?} must be one keyword without list \
                 separators",
                plan.target
            );
            keywords.insert(value.to_owned());
            ensure!(
                keywords.len() <= MAX_KEYWORDS,
                "{} declares more than {MAX_KEYWORDS} keywords",
                plan.target
            );
        }
        Ok(keywords)
    }

    /// The top-level Typst `set document` rule.
    pub(super) fn typst(&self) -> String {
        let list = |values: &mut dyn Iterator<Item = &String>| {
            values
                .map(|value| format!("{}, ", quote(value)))
                .collect::<String>()
        };
        format!(
            "#set document(title: {}, author: ({}), keywords: ({}), date: \
             {})\n",
            quote(&self.title),
            list(&mut self.authors.iter()),
            list(&mut self.keywords.iter()),
            self.date
                .map_or_else(|| "none".to_owned(), PublicationDate::typst)
        )
    }
}
