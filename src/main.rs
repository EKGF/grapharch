use {
    console::style,
    grapharch::{
        DocumentationModel,
        OWLSource,
        TypstGenerator,
        setup_tracing,
    },
    tracing::{error, info},
};

async fn run() -> anyhow::Result<()> {
    setup_tracing()?;

    // Read the OWL file
    let owl_url = "https://ekgf.github.io/dprod/dprod.ttl";
    let mut owl_source = OWLSource::new(owl_url).map_err(|e| {
        error!(
            "{}: {}. URL: {}",
            style("Failed to initialize OWLSource").red(),
            e,
            owl_url
        );

        anyhow::Error::msg(format!("{}: URL: {}", e, owl_url))
    })?;

    // Process the OWL file
    let mut doc_model = DocumentationModel::new()?;
    owl_source.analyze(&mut doc_model).await?;

    // Generate output
    let typst_gen = TypstGenerator::new("output");
    typst_gen.generate(doc_model.get_store())?;

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
