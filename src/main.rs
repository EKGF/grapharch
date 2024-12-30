use {
    grapharch::{
        generator::DocumentationGenerator,
        loader::{LoaderImplementor, MarkdownLoader, RDFLoader},
        model::DocumentationModel,
        source::{FileSourceImplementor, FileSourceVariant},
        store::LoaderStore,
        util::setup_tracing,
    },
    std::path::Path,
    tracing::{error, info},
};

async fn run() -> anyhow::Result<()> {
    setup_tracing()?;

    let file_source = FileSourceImplementor::new(
        FileSourceVariant::FileSystem,
        Some(Path::new(".")),
        None,
    )?;

    let generator = DocumentationGenerator::new(
        vec![
            LoaderImplementor::MarkdownLoader(MarkdownLoader {}),
            LoaderImplementor::RDFLoader(RDFLoader {}),
        ],
        LoaderStore::new_in_memory()?,
        DocumentationModel::new()?,
    );
    generator.generate_from_file_source(&file_source).await?;

    info!("Documentation generation completed successfully.");
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        error!("Application error: {}", e);
        std::process::exit(1);
    }
}
