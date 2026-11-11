//! Header metadata derived from one configured native source. Only that
//! collector use's own contribution is read, and an explicit definition value
//! for a derived predicate is a conflict, never an override.
use {
    crate::publication::{
        dataset::{Dataset, Stage},
        term,
        view::RevisionView,
        writer::{ContributionStream, GraphWriter},
    },
    anyhow::{Context, Result, bail, ensure},
    oxrdf::{GraphName, NamedNode, Quad, Term},
};

/// Engine processor owning derived header metadata.
pub const PROCESSOR: &str = "urn:grapharch:native-metadata";
const SUBJECTS: usize = 16;
const DCT: &str = "http://purl.org/dc/terms/";

/// The fixed mapping from a derived header predicate to its native predicate.
struct Mapping {
    header: NamedNode,
    native: NamedNode,
}

impl Mapping {
    fn of(header: &NamedNode) -> Result<Self> {
        let native = match header.as_str() {
            "http://purl.org/dc/terms/title" => "title",
            "http://purl.org/dc/terms/description" => "description",
            "https://ekgf.org/ontology/grapharch#license" => "license",
            "https://ekgf.org/ontology/grapharch#publicationDate" => "issued",
            _ => {
                bail!(
                    "ga:derivedMetadata {header} has no native mapping; \
                     supported: dct:title, dct:description, ga:license, \
                     ga:publicationDate"
                )
            },
        };
        Ok(Self {
            header: header.clone(),
            native: NamedNode::new(format!("{DCT}{native}"))?,
        })
    }
}

/// Write derived header values for every definition subject that names a
/// `ga:metadataSource`.
pub async fn derive(
    dataset: &Dataset,
    definitions: &RevisionView,
    project: &NamedNode,
    writer: &GraphWriter,
) -> Result<()> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(project.as_str()).await?,
    );
    for subject in subjects(dataset, definitions).await? {
        Header {
            dataset,
            definitions,
            view: &view,
            project,
            subject: &subject,
        }
        .write(writer)
        .await
        .with_context(|| format!("derived native metadata of {subject}"))?;
    }
    Ok(())
}

async fn subjects(
    dataset: &Dataset,
    definitions: &RevisionView,
) -> Result<Vec<NamedNode>> {
    let pattern = definitions.pattern("?subject ?predicate ?value", 0);
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?subject WHERE {{ VALUES ?predicate {{ {} {} }} \
             {pattern} }} ORDER BY ?subject LIMIT {}",
            term("metadataSource"),
            term("derivedMetadata"),
            SUBJECTS + 1
        ))
        .await?;
    let mut subjects = Vec::new(); // bounded: SUBJECTS.
    while let Some(row) = rows.next().await? {
        ensure!(
            subjects.len() < SUBJECTS,
            "native metadata exceeds the {SUBJECTS}-subject budget"
        );
        let Term::NamedNode(subject) = &row["subject"] else {
            bail!("native metadata subjects must be IRIs")
        };
        subjects.push(subject.clone());
    }
    Ok(subjects)
}

struct Header<'a> {
    dataset:     &'a Dataset,
    definitions: &'a RevisionView,
    view:        &'a RevisionView,
    project:     &'a NamedNode,
    subject:     &'a NamedNode,
}

impl Header<'_> {
    async fn write(&self, writer: &GraphWriter) -> Result<()> {
        let source = self
            .definitions
            .node(self.subject, "metadataSource")
            .await
            .context("ga:derivedMetadata requires one ga:metadataSource")?;
        ensure!(
            self.definitions.has_type(&source, "CollectorUse").await?,
            "ga:metadataSource {source} must be a ga:CollectorUse"
        );
        let native = self.definitions.node(&source, "subject").await?;
        let mappings = self.mappings().await?;
        let snapshot = self
            .dataset
            .snapshot_for_processor(self.project.as_str(), source.as_str())
            .await?;
        let native_view =
            RevisionView::new(self.dataset.clone(), snapshot.clone());
        let mut stream = writer
            .begin_stage(
                self.project.as_str(),
                PROCESSOR,
                self.subject.as_str(),
                &format!("native-metadata-v1:{snapshot}"),
                Stage::Document,
            )
            .await?;
        match self
            .values(&native_view, &native, &mappings, &mut stream)
            .await
        {
            Ok(()) => stream.complete().await,
            Err(error) => {
                stream.abort().await?;
                Err(error)
            },
        }
    }

    async fn mappings(&self) -> Result<Vec<Mapping>> {
        let mut rows = self
            .definitions
            .objects(self.subject, "derivedMetadata")
            .await?;
        let mut mappings = Vec::new(); // bounded: four supported predicates.
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(header) = &row["value"] else {
                bail!("ga:derivedMetadata must identify an RDF predicate")
            };
            ensure!(
                mappings.len() < 4,
                "ga:derivedMetadata repeats a mapping"
            );
            mappings.push(Mapping::of(header)?);
        }
        ensure!(
            !mappings.is_empty(),
            "ga:metadataSource requires at least one ga:derivedMetadata"
        );
        mappings.sort_by(|a, b| a.header.cmp(&b.header));
        Ok(mappings)
    }

    async fn values(
        &self,
        native_view: &RevisionView,
        native: &NamedNode,
        mappings: &[Mapping],
        stream: &mut ContributionStream,
    ) -> Result<()> {
        for mapping in mappings {
            ensure!(
                self.view
                    .objects_by(self.subject, &mapping.header)
                    .await?
                    .next()
                    .await?
                    .is_none(),
                "derived {} conflicts with an explicit value on {}",
                mapping.header,
                self.subject
            );
            let mut values =
                native_view.objects_by(native, &mapping.native).await?;
            let value = values.next().await?.with_context(|| {
                format!(
                    "native source {native} is missing {} for derived {}",
                    mapping.native, mapping.header
                )
            })?;
            ensure!(
                values.next().await?.is_none(),
                "native source {native} has multiple {} values for derived {}",
                mapping.native,
                mapping.header
            );
            let value: Term = value["value"].clone();
            stream
                .write(Quad::new(
                    self.subject.clone(),
                    mapping.header.clone(),
                    value,
                    GraphName::DefaultGraph,
                ))
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
