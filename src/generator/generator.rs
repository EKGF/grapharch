use {
    crate::{
        documentor::{Documentor, DocumentorImplementor},
        loader::{Loader, LoaderImplementor},
        model::DocumentationModel,
        source::{FileSource, FileSourceImplementor},
        store::LoaderStore,
        util::{FileType, relative_path},
    },
    std::{collections::HashSet, path::PathBuf},
};

pub struct DocumentationGenerator {
    loaders:      Vec<LoaderImplementor>,
    loader_store: LoaderStore,
    doc_model:    DocumentationModel,
}

impl DocumentationGenerator {
    pub fn new(
        loaders: Vec<LoaderImplementor>,
        loader_store: LoaderStore,
        doc_model: DocumentationModel,
    ) -> Self {
        Self { loaders, loader_store, doc_model }
    }

    pub fn file_types(&self) -> Vec<&'static FileType> {
        tracing::info!("Getting file types for loaders:");
        for loader in &self.loaders {
            tracing::info!("- {:}", *loader);
            for file_type in loader.file_types() {
                tracing::info!("  - {:}", *file_type);
            }
        }
        let file_types_set: HashSet<&'static FileType> = self
            .loaders
            .iter()
            .flat_map(|loader_impl| loader_impl.file_types())
            .map(|file_type| *file_type)
            .collect();
        let file_types: Vec<&'static FileType> =
            file_types_set.into_iter().collect();
        file_types
    }

    /// Scans the given file source for files with the given file
    /// types that the loaders can handle.
    async fn file_names(
        &self,
        source: &FileSourceImplementor,
    ) -> anyhow::Result<Vec<PathBuf>> {
        source.scan(&self.file_types()).await
    }

    /// Load the files into the loader store and collect, from the
    /// loaders, the documentors that can handle the given file
    /// types.
    async fn documentors(
        &self,
        source: &FileSourceImplementor,
        file_names: &Vec<&PathBuf>,
    ) -> anyhow::Result<Vec<DocumentorImplementor>> {
        let documentors: Vec<DocumentorImplementor> =
            futures::future::try_join_all(self.loaders.iter().map(
                |loader| {
                    loader.load_files(
                        &source,
                        file_names,
                        self.loader_store.clone(),
                        self.doc_model.clone(),
                    )
                },
            ))
            .await?
            .into_iter()
            .flatten()
            .collect();

        tracing::info!("We found {} documentors", documentors.len());

        for documentor in documentors.iter() {
            if let Some(file_name) = documentor.file_name() {
                tracing::info!(
                    "Documentor file name: {:}",
                    relative_path(
                        file_name,
                        source.root_path().unwrap()
                    )
                    .display()
                );
            }
        }
        Ok(documentors)
    }

    pub async fn generate_from_file_source(
        &self,
        source: &FileSourceImplementor,
    ) -> anyhow::Result<()> {
        // First, we need to get the file names that the loaders can
        // handle
        let file_names = self.file_names(source).await?;
        let file_names_by_ref: Vec<&PathBuf> =
            file_names.iter().collect();

        // Next, we need to load the files into the loader store and
        // collect, from the loaders, the documentors that can handle
        // the given file types.
        let documentors =
            self.documentors(source, &file_names_by_ref).await?;

        // Finally, we need to generate the documentatable items into
        // the doc_model, using the documentors. This is the
        // only step where a Documentor is allowed
        // to mutate the doc_model.
        futures::future::try_join_all(
            documentors
                .iter()
                .map(|documentor| documentor.generate()),
        )
        .await?;

        Ok(())
    }
}
