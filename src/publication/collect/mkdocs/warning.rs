//! Typed import diagnostics for MkDocs settings that GraphArch reads as data
//! but does not reproduce exactly.
use std::fmt;

/// Why a source-owned MkDocs setting produced a diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum WarningCode {
    /// A presentation setting is replaced by GraphArch's own presentation.
    PresentationSubstitution,
    /// A YAML tag outside the known Material/pymdownx values was ignored.
    UnknownYamlTag,
    /// Python, JavaScript or a template named by the configuration is never
    /// executed or injected.
    NotExecuted,
    /// A plugin is replaced by a built-in GraphArch capability.
    NativeEquivalent,
}
impl WarningCode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::PresentationSubstitution => {
                "mkdocs-presentation-substitution"
            },
            Self::UnknownYamlTag => "mkdocs-unknown-yaml-tag",
            Self::NotExecuted => "mkdocs-not-executed",
            Self::NativeEquivalent => "mkdocs-native-equivalent",
        }
    }
}

/// One import diagnostic: a typed code and a human-readable explanation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ImportWarning {
    pub code:    WarningCode,
    pub message: String,
}
impl ImportWarning {
    pub(super) fn new(code: WarningCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub(super) fn substituted(message: impl Into<String>) -> Self {
        Self::new(WarningCode::PresentationSubstitution, message)
    }

    pub(super) fn not_executed(message: impl Into<String>) -> Self {
        Self::new(WarningCode::NotExecuted, message)
    }

    pub(super) fn native(message: impl Into<String>) -> Self {
        Self::new(WarningCode::NativeEquivalent, message)
    }
}
impl fmt::Display for ImportWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}
