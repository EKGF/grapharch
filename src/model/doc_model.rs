use oxigraph::{model::Quad, store::Store};

pub struct DocumentationModel {
    store: Store,
}

impl DocumentationModel {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self { store: Store::new()? })
    }

    pub async fn add_documentable_item(
        &mut self,
        quad: Quad,
    ) -> anyhow::Result<()> {
        self.store.insert(&quad)?;
        Ok(())
    }

    pub fn get_store(&self) -> &Store { &self.store }
}
