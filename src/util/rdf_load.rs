use {
    console::style,
    futures::TryStreamExt,
    oxigraph::store::Store,
    oxrdf::{GraphName, GraphNameRef},
    oxrdfio::{RdfFormat, RdfParser},
    reqwest::Client,
    std::{path::Path, sync::Arc},
    tokio::{fs::File as AsyncFile, io::AsyncReadExt, task},
    tracing::info,
    url::Url,
};

/// Loads an ontology from a file into a store.
pub async fn rdf_load(
    store: Arc<Store>,
    file_path: &String,
    base_iri: String,
    graph: GraphNameRef<'_>,
) -> anyhow::Result<()> {
    if let Ok(url) = Url::parse(file_path) {
        rdf_load_from_url(store, url, base_iri, graph).await?;
    } else {
        rdf_load_from_file(store, file_path, base_iri, graph).await?;
    }

    Ok(())
}

/// Loads an ontology from a URL into a store.
async fn rdf_load_from_url(
    store: Arc<Store>,
    url: Url,
    base_iri: String,
    graph: GraphNameRef<'_>,
) -> anyhow::Result<()> {
    info!(
        "{} {}...",
        style("Loading ontology from URL").green().bold(),
        style(url.clone()).blue().bold()
    );

    let client = Client::new();
    let response = client.get(url).send().await.map_err(|e| {
        anyhow::Error::new(e).context("Failed to send request")
    })?;
    if !response.status().is_success() {
        return Err(anyhow::Error::msg(format!(
            "Failed to fetch URL: {}",
            response.status()
        )));
    }
    let mut stream = response.bytes_stream().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, e)
    });
    let mut buffer = Vec::new();
    while let Some(chunk) = stream.try_next().await? {
        buffer.extend_from_slice(&chunk);
    }

    let base_iri = base_iri.clone();
    let graph = GraphName::from(graph);
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let reader = std::io::Cursor::new(buffer);

        // for quad in RdfParser::from_format(RdfFormat::Turtle)
        //     .rename_blank_nodes()
        //     .for_reader(reader)
        // {
        //     match quad {
        //         Ok(quad) => info!("{:?}", quad),
        //         Err(e) => return Err(anyhow::Error::msg(e)),
        //     }
        // }

        store
            .load_from_reader(
                RdfParser::from_format(RdfFormat::Turtle)
                    .with_base_iri(base_iri)
                    .map_err(anyhow::Error::msg)?
                    .without_named_graphs() // No named graph allowed in the input
                    .with_default_graph(graph),
                reader,
            )
            .map_err(anyhow::Error::msg)?;
        Ok(())
    })
    .await??;

    info!(
        "{}",
        style("Ontology loaded successfully.").green().bold()
    );

    Ok(())
}

/// Loads an ontology from a file into a store.
async fn rdf_load_from_file(
    store: Arc<Store>,
    file_path: &String,
    base_iri: String,
    graph: GraphNameRef<'_>,
) -> anyhow::Result<()> {
    info!(
        "{} {}...",
        style("Loading ontology from file").green().bold(),
        file_path
    );

    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(anyhow::Error::msg(format!(
            "File not found: {}",
            file_path
        )));
    }
    let mut file = AsyncFile::open(path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;

    let base_iri = base_iri.clone();
    let graph = GraphName::from(graph);
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let reader = std::io::Cursor::new(buffer);
        store
            .load_from_reader(
                RdfParser::from_format(RdfFormat::Turtle)
                    .with_base_iri(base_iri)
                    .map_err(anyhow::Error::msg)?
                    .without_named_graphs()
                    .with_default_graph(graph),
                reader,
            )
            .map_err(anyhow::Error::msg)?;
        Ok(())
    })
    .await??;

    info!(
        "{}",
        style("Ontology loaded successfully.").green().bold()
    );

    Ok(())
}
