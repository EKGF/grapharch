//! Read the RDF documents a showcase configuration collects.
use {
    anyhow::Result,
    oxigraph::{io::RdfFormat, sparql::QueryResults, store::Store},
};

/// Every `ga:rdfDocument` URL of the configured showcase with its media type.
pub fn documents(configuration: &str) -> Result<Vec<(String, String)>> {
    let store = Store::new()?;
    store.load_from_reader(
        RdfFormat::Turtle,
        std::fs::read(configuration)?.as_slice(),
    )?;
    let QueryResults::Solutions(rows) =
        oxigraph::sparql::SparqlEvaluator::new()
            .parse_query(
                "PREFIX ga: <https://ekgf.org/ontology/grapharch#> SELECT \
                 ?document ?format WHERE { ?showcase a ga:Showcase ; \
                 ga:rdfDocument ?document . OPTIONAL { ?document ga:format \
                 ?format } } ORDER BY ?document",
            )?
            .on_store(&store)
            .execute()?
    else {
        anyhow::bail!("expected solutions")
    };
    // bounded: at most 64 configured documents.
    let mut documents = Vec::new();
    for row in rows {
        let row = row?;
        let Some(oxrdf::Term::NamedNode(document)) = row.get("document") else {
            anyhow::bail!("documents must be IRIs")
        };
        let format = match row.get("format") {
            Some(oxrdf::Term::Literal(format)) => format.value().to_owned(),
            _ => String::new(),
        };
        documents.push((document.as_str().to_owned(), format));
    }
    Ok(documents)
}
