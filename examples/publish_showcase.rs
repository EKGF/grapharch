//! Run the same showcase pipelines exercised by tests/showcases.rs.
mod support;

use {
    clap::Parser,
    grapharch::publication::render::Publisher,
    std::path::{Path, PathBuf},
};

#[derive(Parser)]
struct Arguments {
    /// Collect Markdown, SPARQL, and OWL rather than the authored RDF
    /// showcase.
    #[arg(long, conflicts_with_all = ["specification", "w3c", "legacy_w3c"])]
    mixed:         bool,
    /// Publish the standards-style specification showcase.
    #[arg(long, conflicts_with_all = ["w3c", "legacy_w3c"])]
    specification: bool,
    /// Collect and render the pinned W3C RDF 1.2 N-Triples publication.
    #[arg(long, conflicts_with = "legacy_w3c")]
    w3c:           bool,
    /// Run the retained RDF 1.1 regression showcase.
    #[arg(long)]
    legacy_w3c:    bool,
    /// Continue with explicit diagnostic previews; exit nonzero on errors.
    #[arg(long, conflicts_with = "fail_fast")]
    keep_going:    bool,
    /// Reject the first error (default).
    #[arg(long, conflicts_with = "keep_going")]
    fail_fast:     bool,
    /// Artifact root; each target owns a separate subdirectory.
    output:        Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    let (dataset, plans) = if arguments.w3c {
        support::w3c(Path::new("examples/w3c-rdf12-n-triples")).await?
    } else if arguments.legacy_w3c {
        support::w3c(Path::new("examples/w3c-n-triples")).await?
    } else if arguments.specification {
        support::specification(Path::new("examples/specification/project.ttl"))
            .await?
    } else if arguments.mixed {
        support::mixed(Path::new("examples/mixed-sources")).await?
    } else {
        support::authored(Path::new("examples/showcase/project.ttl")).await?
    };
    let mode = if arguments.keep_going {
        grapharch::publication::diagnostics::FailureMode::KeepGoing
    } else {
        grapharch::publication::diagnostics::FailureMode::FailFast
    };
    let root = arguments.output_root();
    let publisher = Publisher::new(dataset.clone()).with_failure_mode(mode);
    let mut errors = 0;
    for publication in support::publish(&publisher, &plans, &root).await? {
        println!("{}", publication.directory.display());
        for diagnostic in &publication.manifest.diagnostics {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
            errors += 1;
        }
    }
    dataset.export(root.join("documentation.nq")).await?;
    anyhow::ensure!(
        errors == 0,
        "showcase generated with {errors} error(s)"
    );
    Ok(())
}

impl Arguments {
    fn output_root(self) -> PathBuf {
        self.output.unwrap_or_else(|| {
            PathBuf::from(
                if self.w3c {
                    "output/pdf/grapharch-w3c-rdf12-n-triples"
                } else if self.legacy_w3c {
                    "output/pdf/grapharch-w3c-n-triples"
                } else if self.specification {
                    "output/pdf/grapharch-specification"
                } else if self.mixed {
                    "output/pdf/grapharch-mixed-sources"
                } else {
                    "output/pdf/grapharch-showcase"
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn specification_and_authored_showcases_have_separate_default_roots() {
        let authored = Arguments::parse_from(["showcase"]).output_root();
        let specification =
            Arguments::parse_from(["showcase", "--specification"])
                .output_root();
        assert_ne!(authored, specification);
        assert_eq!(
            specification,
            PathBuf::from("output/pdf/grapharch-specification")
        );
    }
}
