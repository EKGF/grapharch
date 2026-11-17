//! Binds the tabular reference of each family ontology when a profile
//! selects a term table, and schedules one table documentor per ontology.
use {
    super::{Member, identity, q},
    crate::publication::{
        dataset::Dataset,
        document::{
            Documentor,
            term_table::{OntologySource, TermTable, TermTableDocumentor},
        },
        writer::GraphWriter,
    },
    anyhow::Result,
    futures::{StreamExt, TryStreamExt},
    oxrdf::{Literal, NamedNode, Quad},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

/// The table documentors of every bound tabular profile.
#[derive(Default)]
pub(super) struct TableJobs {
    // bounded: one job per family ontology and distinct term table.
    jobs: Vec<TermTableDocumentor>,
    keys: HashSet<String>,
}

/// What every table of one family shares.
pub(super) struct FamilyTables<'a> {
    pub dataset:   &'a Dataset,
    pub collected: &'a NamedNode,
    pub project:   &'a NamedNode,
    pub prefixes:  &'a Arc<BTreeMap<String, String>>,
    pub known:     &'a Arc<HashSet<NamedNode>>,
}

impl TableJobs {
    /// Place one ontology's tables in its chapter for `target`, and schedule
    /// the documentor that writes them.
    pub async fn bind(
        &mut self,
        family: &FamilyTables<'_>,
        data: &mut Vec<Quad>,
        target: &NamedNode,
        scope: &NamedNode,
        member: &Member,
        table: &TermTable,
    ) -> Result<()> {
        let introduction =
            super::reference::introduction(&member.view, &member.ontology)
                .await?;
        let documentor = TermTableDocumentor::new(
            family.dataset.clone(),
            family.collected.clone(),
            family.project,
            OntologySource {
                ontology: member.ontology.clone(),
                namespace: member.namespace.clone(),
                title: member.title.clone(),
                introduction,
                snapshot: member.snapshot.clone(),
                terms: member.terms.clone(),
                foreign: member.foreign.clone(),
            },
            (family.prefixes.clone(), family.known.clone()),
            table.clone(),
        );
        let component = identity("family-component", &[
            target.as_str(),
            member.ontology.as_str(),
        ]);
        let fragments = documentor.fragments();
        for (order, fragment) in std::iter::once(fragments.metadata)
            .chain(fragments.details)
            .enumerate()
        {
            data.extend([
                q(&component, "fragment", fragment.clone()),
                q(&fragment, "order", Literal::from(order as u32)),
            ]);
        }
        data.push(q(scope, "include", member.ontology.clone()));
        for subject in &member.terms {
            data.push(q(scope, "include", subject.clone()));
        }
        if self.keys.insert(documentor.source_key()) {
            self.jobs.push(documentor);
        }
        Ok(())
    }

    /// The ownership entries of the scheduled documentors.
    pub fn sources(&self) -> impl Iterator<Item = String> + '_ {
        self.jobs.iter().map(TermTableDocumentor::source_key)
    }

    pub async fn run(
        self,
        writer: &GraphWriter,
        concurrency: usize,
    ) -> Result<()> {
        futures::stream::iter(self.jobs.iter().map(|job| job.document(writer)))
            .buffer_unordered(concurrency)
            .try_collect::<Vec<_>>()
            .await?;
        Ok(())
    }
}
