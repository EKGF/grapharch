use {
    console::style,
    futures::TryStreamExt,
    oxigraph::store::Store,
    oxrdf::{GraphName, GraphNameRef},
    oxrdfio::{RdfFormat, RdfParser},
    reqwest::Client,
    std::path::Path,
    tokio::{fs::File as AsyncFile, io::AsyncReadExt, task},
    tracing::info,
    url::Url,
};

pub async fn rdf_load<'a>(
    mut store: Store,
    file_path: &String,
    base_iri: &String,
    graph: GraphNameRef<'a>,
) -> anyhow::Result<Store> {
    info!(
        "{}",
        style("Loading ontology from source...").green().bold()
    );

    if let Ok(url) = Url::parse(&file_path) {
        // Handle URL
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
        store =
            task::spawn_blocking(move || -> anyhow::Result<Store> {
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
                Ok(store)
            })
            .await??;
    } else {
        // Handle file path
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
        store =
            task::spawn_blocking(move || -> anyhow::Result<Store> {
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
                Ok(store)
            })
            .await??;
    }

    info!(
        "{}",
        style("Ontology loaded successfully.").green().bold()
    );

    Ok(store)
}
