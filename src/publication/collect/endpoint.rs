//! One explicit CONSTRUCT/DESCRIBE request becomes one isolated RDF
//! contribution.
use {
    super::{Collector, SourceIdentity, blank_nodes::BlankNodes},
    crate::publication::{
        sparql::{HttpPolicy, endpoint, http_error, status},
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    futures::StreamExt,
    oxrdf::{GraphName, NamedNode},
    oxrdfio::{RdfFormat, RdfParser},
    serde::Deserialize,
    sha2::{Digest, Sha256},
    tokio::io::{AsyncWriteExt, BufWriter},
};
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SparqlSourceConfig {
    pub endpoint:         String,
    pub query:            String,
    #[serde(default)]
    pub default_graphs:   Vec<String>,
    #[serde(default)]
    pub named_graphs:     Vec<String>,
    #[serde(default)]
    pub source_graph:     Option<String>,
    #[serde(default = "default_limit")]
    pub max_source_bytes: u64,
    #[serde(default)]
    pub http:             HttpPolicy,
}
fn default_limit() -> u64 { 8 * 1024 * 1024 }
pub struct SparqlEndpointSource {
    project:   NamedNode,
    collector: NamedNode,
    config:    SparqlSourceConfig,
}
impl SparqlEndpointSource {
    pub fn new(
        project: &str,
        collector: &str,
        config: SparqlSourceConfig,
    ) -> Result<Self> {
        endpoint(&config.endpoint)?;
        ensure!(
            !config.query.is_empty() && config.query.len() <= 64 * 1024,
            "SPARQL source query must be between 1 and 65536 bytes"
        );
        let query =
            spargebra::SparqlParser::new().parse_query(&config.query)?;
        ensure!(
            matches!(
                query,
                spargebra::Query::Construct { .. } |
                    spargebra::Query::Describe { .. }
            ),
            "SPARQL source requires a CONSTRUCT or DESCRIBE query; query \
             files are documented separately"
        );
        ensure!(
            (1..=128 * 1024 * 1024).contains(&config.max_source_bytes),
            "SPARQL source byte limit must be between 1 and 128 MiB"
        );
        ensure!(
            config.default_graphs.len() + config.named_graphs.len() <= 256,
            "SPARQL source has too many dataset graph parameters"
        );
        for graph in config
            .default_graphs
            .iter()
            .chain(&config.named_graphs)
            .chain(config.source_graph.iter())
        {
            NamedNode::new(graph)?;
        }
        config.http.headers()?;
        Ok(Self {
            project: NamedNode::new(project)?,
            collector: NamedNode::new(collector)?,
            config,
        })
    }
}
#[async_trait]
impl Collector for SparqlEndpointSource {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   self.project.as_str(),
            processor: self.collector.as_str(),
            source:    &self.config.endpoint,
        })
    }

    async fn produce(&self, output: &mut ContributionStream) -> Result<String> {
        let client = self.config.http.asynchronous()?;
        let byte_limit = self
            .config
            .max_source_bytes
            .min(self.config.http.max_response_bytes);
        let mut params = Vec::new();
        for graph in &self.config.default_graphs {
            params.push(("default-graph-uri", graph));
        }
        for graph in &self.config.named_graphs {
            params.push(("named-graph-uri", graph));
        }
        let response = client
            .post(&self.config.endpoint)
            .query(&params)
            .header("Content-Type", "application/sparql-query")
            .header(
                "Accept",
                "application/n-quads, application/n-triples;q=0.9, \
                 text/turtle;q=0.8, application/rdf+xml;q=0.7",
            )
            .body(self.config.query.clone())
            .send()
            .await
            .map_err(http_error)?;
        status(response.status())?;
        ensure!(
            response
                .content_length()
                .is_none_or(|size| size <= byte_limit),
            "SPARQL source exceeds configured byte limit"
        );
        let mime = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "SPARQL source response requires an RDF Content-Type"
                )
            })?
            .split(';')
            .next()
            .unwrap()
            .trim();
        let format = RdfFormat::from_media_type(mime).ok_or_else(|| {
            anyhow::anyhow!(
                "SPARQL source response is not a supported RDF serialization"
            )
        })?;
        ensure!(
            response
                .headers()
                .get("Content-Encoding")
                .is_none_or(|v| v == "identity"),
            "SPARQL source uses unsupported content encoding"
        );
        // A bounded on-disk spool permits revision-scoped, deterministic blank
        // nodes without buffering a graph or trusting an optional server ETag.
        let spool = tempfile::NamedTempFile::new()?;
        let file = tokio::fs::File::from_std(spool.reopen()?);
        let mut writer = BufWriter::new(file);
        let mut hash = Sha256::new();
        hash.update(format!(
            "{}\n{}\n{:?}\n{:?}\n{:?}\n{format:?}",
            self.config.endpoint,
            self.config.query,
            self.config.default_graphs,
            self.config.named_graphs,
            self.config.source_graph
        ));
        let mut bytes = 0u64;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(http_error)?;
            bytes += chunk.len() as u64;
            ensure!(
                bytes <= byte_limit,
                "SPARQL source exceeds configured byte limit"
            );
            hash.update(&chunk);
            writer.write_all(&chunk).await?;
        }
        writer.flush().await?;
        drop(writer);
        let revision = format!(
            "sparql-source-v1:{}",
            hex::encode(hash.finalize())
        );
        let mut blanks = BlankNodes::new(
            self.project.as_str(),
            self.collector.as_str(),
            &self.config.endpoint,
            &revision,
        );
        let file = tokio::fs::File::open(spool.path()).await?;
        let mut parser = RdfParser::from_format(format)
            .with_base_iri(&self.config.endpoint)?
            .for_tokio_async_reader(tokio::io::BufReader::new(file));
        while let Some(quad) = parser.next().await {
            let mut quad = quad?;
            if quad.graph_name == GraphName::DefaultGraph &&
                let Some(graph) = &self.config.source_graph
            {
                quad.graph_name = NamedNode::new(graph)?.into();
            }
            output.write(blanks.normalize(quad)).await?;
        }
        Ok(revision)
    }
}
