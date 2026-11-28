//! SPARQL evaluation against the mock repository.
use {
    anyhow::Result,
    oxigraph::{
        sparql::{
            QueryResults,
            SparqlEvaluator,
            results::{QueryResultsFormat, QueryResultsSerializer},
        },
        store::Store,
    },
};
pub(super) fn query_json(store: &Store, query: &str) -> Result<Vec<u8>> {
    let serializer =
        QueryResultsSerializer::from_format(QueryResultsFormat::Json);
    match SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(store)
        .execute()?
    {
        QueryResults::Boolean(value) => {
            Ok(serializer.serialize_boolean_to_writer(Vec::new(), value)?)
        },
        QueryResults::Solutions(rows) => {
            let mut writer = serializer.serialize_solutions_to_writer(
                Vec::new(),
                rows.variables().to_vec(),
            )?;
            for row in rows {
                writer.serialize(row?.iter())?;
            }
            Ok(writer.finish()?)
        },
        QueryResults::Graph(_) => {
            anyhow::bail!("protocol mock requires SELECT or ASK")
        },
    }
}
