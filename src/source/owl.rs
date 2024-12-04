use {
    crate::{model::doc_model::DocumentationModel, rdf_load},
    console::style,
    oxigraph::{model::*, store::Store},
    std::path::Path,
    tracing::{error, info},
};

pub struct OWLSource {
    /// A temporary store that just holds the data from the source
    store:     Store,
    file_path: String,
    graph:     GraphName,
    base_iri:  String,
}

impl OWLSource {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        info!(
            "{}",
            style("Starting OWL source...").green().bold()
        );
        Ok(Self {
            store:     Store::new()?,
            file_path: path.as_ref().to_string_lossy().into_owned(),
            graph:     NamedNodeRef::new("http://example.com/g2")?
                .into(),
            base_iri:  "http://example.com".to_string(),
        })
    }

    /// Load the data from the source into the store.
    /// Note that this is not the same store as the store held by the
    /// documentation model. This store is temporary and is used to
    /// hold the data from the source.
    async fn load(&mut self) -> anyhow::Result<()> {
        let new_store = Store::new()?;
        self.store = rdf_load(
            std::mem::replace(&mut self.store, new_store),
            &self.file_path,
            &self.base_iri,
            self.graph.as_ref(),
        )
        .await?;

        Ok(())
    }

    /// Analyze the ontology and give the documentation items to the
    /// given DocumentationModel.
    pub async fn analyze(
        &mut self,
        doc_model: &mut DocumentationModel,
    ) -> anyhow::Result<()> {
        info!(
            "{}",
            style("Analyzing ontology...").green().bold()
        );
        self.load().await?;

        let mut documentable_items = Vec::new();
        for quad_result in self.store.quads_for_pattern(
            None,
            None,
            None,
            Some(self.graph.as_ref()),
        ) {
            match quad_result {
                Ok(quad) => {
                    if self.is_documentable(&quad) {
                        documentable_items.push(quad);
                    }
                },
                Err(e) => {
                    error!(
                        "{}: {}",
                        style("Error processing quad").red().bold(),
                        e
                    );
                    continue;
                },
            }
        }
        // Store in documentation model
        for item in documentable_items {
            doc_model.add_documentable_item(item).await?;
        }

        Ok(())
    }

    fn is_documentable(&self, _quad: &Quad) -> bool {
        // TODO: Implement logic to determine if a
        // triple represents something we should
        // document For example: Classes,
        // Properties, Labels, Comments, etc.
        true
    }
}
