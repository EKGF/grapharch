//! The Markdown bodies of one ontology's metadata and details tables.
use {
    super::{
        annotations::AnnotationLabels,
        documentor::TermTableDocumentor,
        markdown::{TableLabel, TableMarkdown},
        metadata::OntologyMetadata,
        pattern::TextPattern,
        row::{PropertyKinds, RowContext, TermRow},
    },
    crate::publication::{anchor::SubjectAnchor, view::RevisionView},
    anyhow::Result,
};

impl TermTableDocumentor {
    pub(super) fn lead_in(
        &self,
        pattern: Option<&TextPattern>,
        caption: &str,
        label: &TableLabel,
        markdown: &mut TableMarkdown,
    ) {
        if let Some(pattern) = pattern {
            markdown.paragraph(
                &pattern.markdown(&self.title, &label.reference(caption)),
            );
        }
    }

    pub(super) async fn metadata(
        &self,
        source: &RevisionView,
    ) -> Result<String> {
        let mut markdown = TableMarkdown::new();
        if !self.introduction.trim().is_empty() {
            markdown.paragraph(&self.introduction);
        }
        let caption = self.table.metadata_caption.text(&self.title);
        let label = TableLabel::new(&self.ontology, "metadata");
        self.lead_in(
            self.table.metadata_lead_in.as_ref(),
            &caption,
            &label,
            &mut markdown,
        );
        markdown.figure(caption, &label)?;
        OntologyMetadata::read(source, &self.ontology)
            .await?
            .write(
                &mut markdown,
                &self.names,
                self.table.metadata.as_deref(),
            )?;
        markdown.end()?;
        markdown.finish()
    }

    pub(super) async fn rows(
        &self,
        source: &RevisionView,
        collected: &RevisionView,
    ) -> Result<Vec<(usize, TermRow)>> {
        let context = RowContext {
            dataset: &self.dataset,
            source,
            collected,
            names: &self.names,
            annotations: self.table.annotations.as_deref(),
        };
        let mut labels = AnnotationLabels::default();
        let mut properties = PropertyKinds::default();
        let mut rows = Vec::new(); // bounded: terms and foreign subjects.
        for (subject, own) in self
            .terms
            .iter()
            .map(|subject| (subject, true))
            .chain(self.foreign.iter().map(|subject| (subject, false)))
        {
            let anchor = own.then(|| SubjectAnchor::new(subject).markdown());
            let row = TermRow::read(
                &context,
                &mut labels,
                &mut properties,
                subject,
                anchor,
            )
            .await?;
            let section =
                self.table.section_of(&row.types).ok_or_else(|| {
                    anyhow::anyhow!(
                        "{subject} has no section in term table {}; add a \
                         ga:referenceSection whose ga:termType it declares",
                        self.table.identity()
                    )
                })?;
            rows.push((section, row));
        }
        rows.sort_by_cached_key(|(section, row)| {
            (
                *section,
                self.names.name(&row.subject).to_lowercase(),
                row.subject.clone(),
            )
        });
        Ok(rows)
    }

    pub(super) fn details(&self, rows: &[(usize, TermRow)]) -> Result<String> {
        let mut markdown = TableMarkdown::new();
        let caption = self.table.details_caption.text(&self.title);
        let label = TableLabel::new(&self.ontology, "details");
        self.lead_in(
            self.table.details_lead_in.as_ref(),
            &caption,
            &label,
            &mut markdown,
        );
        markdown.figure(caption, &label)?;
        for (index, section) in self.table.sections.iter().enumerate() {
            let members: Vec<_> = rows
                .iter()
                .filter(|(s, _)| *s == index)
                .map(|(_, row)| row)
                .collect();
            let Some(first) = members.first() else {
                continue
            };
            let column = if members
                .iter()
                .all(|row| row.kind.axiom_column() == first.kind.axiom_column())
            {
                first.kind.axiom_column()
            } else {
                "Axioms"
            };
            markdown.paragraph(&format!(
                "**{}**",
                super::super::annotated_term::escape(&section.title)
            ));
            markdown.table(&[
                ("Name", 24),
                ("Annotations", 44),
                (column, 32),
            ])?;
            for row in members {
                for cell in row.cells() {
                    markdown.cell(&cell)?;
                }
            }
            markdown.end()?;
        }
        markdown.end()?;
        markdown.finish()
    }
}
