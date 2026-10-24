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
    tracing::info,
};

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    setup_tracing(cli.verbose)?;

    cli.log_info();

    cli.check_for_help();

    if let Some(Commands::Publication { command }) = &cli.command {
        anyhow::ensure!(
            cli.file.is_none() && cli.config.is_none(),
            "publication commands use their own explicit input arguments"
        );
        let mode = if cli.keep_going {
            grapharch::publication::diagnostics::FailureMode::KeepGoing
        } else {
            grapharch::publication::diagnostics::FailureMode::FailFast
        };
        return grapharch::publication::cli::run(command, mode).await;
    }

    anyhow::ensure!(
        !cli.keep_going,
        "--keep-going is supported by the publication pipeline, not the \
         legacy generation prototype"
    );
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
    let file_source = FileSourceImplementor::new(
        FileSourceVariant::FileSystem,
        cli.file.as_deref().or(Some(Path::new("."))),
        None,
    )?;
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
                    .cloned()
                    .unwrap_or_else(|| default_pdf_path.clone());

                let mut typst_gen = if let Some(dir) = typst_dir {
                    TypstGenerator::new(dir.clone(), pdf_path)
                } else {
                    TypstGenerator::with_temp_dir(pdf_path)?
                };
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
        Some(Commands::Publication { .. }) => {
            anyhow::bail!("publication command was not dispatched")
        },
        None => {},
    }

    info!("Documentation generation completed successfully.");
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
