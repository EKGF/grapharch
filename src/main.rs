use {
    clap::{CommandFactory, Parser},
    grapharch::{
        cli::{Cli, Commands},
        generator::{
            ConsoleGenerator,
            DocumentationGenerator,
            Generator,
            TypstGenerator,
        },
        loader::{LoaderImplementor, MarkdownLoader, RDFLoader},
        model::Model,
        source::{FileSourceImplementor, FileSourceVariant},
        store::LoaderStore,
        util::setup_tracing,
    },
    std::{path::Path, sync::Arc},
    tracing::{error, info},
};

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Show help and exit if no command is provided
    if cli.command.is_none() {
        Cli::command().print_help()?;
        std::process::exit(0);
    }

    setup_tracing(cli.verbose)?;

    // You can check the value provided by positional arguments, or option
    // arguments
    if let Some(file) = cli.file.as_deref() {
        println!("Value for file: {}", file.display());
    }

    if let Some(config_path) = cli.config.as_deref() {
        println!("Value for config: {}", config_path.display());
    }

    // You can see how many times a particular flag or argument occurred
    // Note, only flags can have multiple occurrences
    match cli.debug {
        0 => println!("Debug mode is off"),
        1 => println!("Debug mode is kind of on"),
        2 => println!("Debug mode is on"),
        _ => println!("Don't be crazy"),
    }

    let file_source = FileSourceImplementor::new(
        FileSourceVariant::FileSystem,
        cli.file.as_deref().or(Some(Path::new("."))),
        None,
    )?;

    let doc_model = Arc::new(Model::new()?);
    let loader_store = LoaderStore::new_in_memory()?;

    let generator = DocumentationGenerator::new(
        vec![
            LoaderImplementor::MarkdownLoader(MarkdownLoader {}),
            LoaderImplementor::RDFLoader(RDFLoader {}),
        ],
        loader_store.clone(),
        doc_model.clone(),
    );

    // Process the input files
    generator.generate_from_file_source(&file_source).await?;

    // Handle output generation based on command
    match &cli.command {
        Some(Commands::Test { list }) => {
            if *list {
                println!("Printing testing lists...");
            } else {
                println!("Not printing testing lists...");
            }
        },
        Some(Commands::Generate { console, typst, pdf, markdown, html }) => {
            // Check if any output format is explicitly specified
            let any_output_specified = *console ||
                typst.is_some() ||
                pdf.is_some() ||
                markdown.is_some() ||
                html.is_some();

            // Check if we have any input to process
            let has_input = cli.file.is_some();

            // If no input and no output format specified, show help and exit
            if !has_input && !any_output_specified {
                Cli::command().print_help()?;
                std::process::exit(0);
            }

            // Use console output if explicitly requested or if no specific
            // output format is specified
            if *console ||
                (!typst.is_some() &&
                    !pdf.is_some() &&
                    !markdown.is_some() &&
                    !html.is_some())
            {
                let mut console_gen = ConsoleGenerator::new();
                console_gen.generate(doc_model.clone())?;
            }

            // Handle Typst/PDF generation
            if typst.is_some() || pdf.is_some() {
                // Convert the nested Options to single Options with defaults
                let typst_dir = typst.as_ref().and_then(|t| t.as_ref());

                // Create a longer-lived PathBuf for the default value
                let default_pdf_path = std::path::PathBuf::from("output.pdf");
                let pdf_path = pdf
                    .as_ref()
                    .and_then(|p| p.as_ref())
                    .or(Some(&default_pdf_path));

                let mut typst_gen = TypstGenerator::new(typst_dir, pdf_path)?;
                typst_gen.generate(doc_model.clone())?;
            }

            if let Some(output_dir) = markdown {
                // TODO: Implement MarkdownGenerator
                info!(
                    "Markdown generation to {} not yet implemented",
                    output_dir.display()
                );
            }
            if let Some(output_dir) = html {
                // TODO: Implement HTMLGenerator
                info!(
                    "HTML generation to {} not yet implemented",
                    output_dir.display()
                );
            }
        },
        None => {},
    }

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
