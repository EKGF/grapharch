//! Typed diagnostics for mdBook settings that GraphArch does not execute.
use std::fmt;

/// An mdBook setting retained as source metadata instead of being acted on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MdBookWarning {
    /// A `[preprocessor.<name>]` table; mdBook would run a program.
    PreprocessorNotExecuted(String),
    /// An `[output.<name>]` backend other than HTML.
    OutputNotProduced(String),
    /// An `[output.html]` or other presentation option.
    PresentationSubstitution(String),
}

impl MdBookWarning {
    /// The stable diagnostic code recorded in the documentation graph.
    pub(super) fn code(&self) -> &'static str {
        match self {
            Self::PreprocessorNotExecuted(_) => {
                "mdbook-preprocessor-not-executed"
            },
            Self::OutputNotProduced(_) => "mdbook-output-not-produced",
            Self::PresentationSubstitution(_) => {
                "mdbook-presentation-substitution"
            },
        }
    }
}

impl fmt::Display for MdBookWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreprocessorNotExecuted(name) => {
                write!(
                    f,
                    "preprocessor.{name}: not executed; GraphArch never runs \
                     mdBook preprocessors"
                )
            },
            Self::OutputNotProduced(name) => {
                write!(
                    f,
                    "output.{name}: not produced; GraphArch publishes its own \
                     website"
                )
            },
            Self::PresentationSubstitution(option) => {
                write!(
                    f,
                    "{option}: retained as source metadata; GraphArch \
                     controls this presentation setting"
                )
            },
        }
    }
}
