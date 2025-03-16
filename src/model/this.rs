use {
    oxigraph::{model::Quad, store::Store},
    std::sync::Arc,
};

/// The documentation model holds the data that needs to be
/// documented. It is a wrapper around an OxiGraph store.
/// The store does not necessarily hold all data from the source, but
/// only the data that's needed by the output generators to generate
/// the documentation. This documentation data's structure is
/// primarily defined by the GraphArch ontology.
/// TODO: Change the field book_nodes to books, and section_nodes to sections
/// TODO: Change the field books to a map of Books, not NamedNodes
/// TODO: Change the field sections to a map of Sections, not NamedNodes
/// TODO: Move the field sections to the Book struct
#[derive(Clone)]
pub struct Model {
    store: Arc<Store>,
}

impl Model {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self { store: Arc::new(Store::new()?) })
    }

    pub(super) fn get_store(&self) -> &Arc<Store> { &self.store }

    pub(super) fn insert(&self, quad: &Quad) -> anyhow::Result<()> {
        self.store.insert(quad)?;
        Ok(())
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DocumentationModel")
    }
}
