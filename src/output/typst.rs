use {oxigraph::store::Store, std::path::Path};

pub struct TypstGenerator {
    #[allow(dead_code)]
    output_dir: String,
}

impl TypstGenerator {
    pub fn new<P: AsRef<Path>>(output_dir: P) -> Self {
        Self {
            output_dir: output_dir.as_ref().to_string_lossy().into_owned(),
        }
    }

    pub fn generate(&self, _store: &Store) -> anyhow::Result<()> {
        // TODO: Query the store and generate Typst documentation
        Ok(())
    }
}
