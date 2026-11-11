//! Engine-owned shared definitions retained alongside project source
//! contributions.
use {
    super::dataset::{Dataset, Stage},
    anyhow::Result,
    oxrdfio::{RdfFormat, RdfParser},
    sha2::{Digest, Sha256},
};

fn sources() -> [(&'static str, &'static str); 3] {
    [
        (
            "renderers",
            include_str!("../../docs/other/ontology/renderers.ttl"),
        ),
        (
            "collectors",
            include_str!("../../docs/other/ontology/collectors.ttl"),
        ),
        (
            "documentors",
            include_str!("../../docs/other/ontology/documentors.ttl"),
        ),
    ]
}

pub(crate) fn load(store: &oxigraph::store::Store) -> Result<()> {
    for (_, source) in sources() {
        store.load_from_reader(RdfFormat::Turtle, source.as_bytes())?;
    }
    Ok(())
}

pub(crate) async fn retain(dataset: &Dataset, project: &str) -> Result<()> {
    for (name, source) in sources() {
        let revision = hex::encode(Sha256::digest(source.as_bytes()));
        let attempt = dataset
            .begin_stage(
                project,
                "urn:grapharch:processor:definition-catalogue",
                &format!(
                    "https://ekgf.org/ontology/grapharch/catalogue/{name}"
                ),
                &revision,
                Stage::Collect,
            )
            .await?;
        // bounded: compile-time catalogue contains only the supported engine
        // definitions.
        for quad in RdfParser::from_format(RdfFormat::Turtle)
            .for_slice(source.as_bytes())
        {
            attempt.push(vec![quad?]).await?;
        }
        attempt.complete().await?;
    }
    Ok(())
}
