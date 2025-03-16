use {clap::Subcommand, std::path::PathBuf};

#[derive(Subcommand)]
pub enum Commands {
    /// does testing things
    Test {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },
    /// Generate documentation in various formats
    Generate {
        /// Generate console output
        #[arg(long)]
        console: bool,

        /// Generate Typst source files. When flag is present without value,
        /// uses a temporary directory
        #[arg(long, value_name = "DIR", num_args = 0..=1)]
        typst: Option<Option<PathBuf>>,

        /// Generate PDF documentation using Typst. When flag is present
        /// without value, uses './output.pdf'
        #[arg(long, value_name = "FILE", num_args = 0..=1)]
        pdf: Option<Option<PathBuf>>,

        /// Generate Markdown documentation
        #[arg(long, value_name = "DIR")]
        markdown: Option<PathBuf>,

        /// Generate HTML documentation
        #[arg(long, value_name = "DIR")]
        html: Option<PathBuf>,
    },
}
