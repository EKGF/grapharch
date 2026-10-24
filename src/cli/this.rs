use {
    super::Commands,
    clap::{CommandFactory, Parser},
    std::path::PathBuf,
    tracing::debug,
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// In case you have one particular input file, such as an OWL ontology,
    /// specify the path to the file
    #[arg(short, long, value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Sets a custom config file (not yet implemented)
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Stop on the first error (the default).
    #[arg(long, global = true, conflicts_with = "keep_going")]
    pub fail_fast: bool,

    /// Continue supported work, report errors, and exit nonzero if any occur.
    #[arg(long, global = true, conflicts_with = "fail_fast")]
    pub keep_going: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub fn log_info(&self) {
        if self.verbose.is_present() {
            debug!("The user specified the verbose or silent flag");
            debug!("log level: {:?}", self.verbose.log_level());
        }
        // You can check the value provided by positional arguments, or option
        // arguments
        if let Some(file) = self.file.as_deref() {
            debug!("Input file specified: {}", file.display());
        }

        if let Some(config_path) = self.config.as_deref() {
            debug!("Value for config: {}", config_path.display());
        }
    }

    /// Show help and exit if no command is provided
    pub fn check_for_help(&self) {
        if self.command.is_none() {
            let _ = <Self as CommandFactory>::command().print_help();
            std::process::exit(0);
        }
    }
}
