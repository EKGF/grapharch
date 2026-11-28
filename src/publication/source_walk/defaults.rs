//! Paths no repository means to publish unless a project includes them.
//!
//! Agent instructions, planning and status notes, contribution templates and
//! test fixtures live beside documentation in most code repositories. They are
//! skipped by default; `ga:includeSourcePattern` brings any of them back, and
//! `ga:excludeSourcePattern` adds more.
use {
    crate::publication::website::exclusions::{EntryKind, SourcePattern},
    anyhow::Result,
};

/// Default exclusion globs, relative to the discovery root.
pub(crate) const PATTERNS: &[&str] = &[
    // Agent and assistant instructions.
    "**/CLAUDE.md",
    "**/AGENTS.md",
    "**/GEMINI.md",
    "**/COPILOT.md",
    "**/.cursorrules",
    // Planning and status notes.
    "**/PLAN.md",
    "**/TODO.md",
    "**/*_STATUS.md",
    "**/*-STATUS.md",
    // Forge templates.
    "**/pull_request_template.md",
    "**/PULL_REQUEST_TEMPLATE.md",
    "**/ISSUE_TEMPLATE/",
    // Test fixtures and snapshots.
    "**/fixtures/",
    "**/testdata/",
    "**/test_data/",
    "**/test-data/",
    "**/__snapshots__/",
    "**/tests/",
    "test/",
    "**/__tests__/",
    "**/spec/**/*.{ttl,rdf,nt,nq,trig,n3,jsonld}",
    // Dependencies and build output.
    "**/node_modules/",
    // Build output is anchored at the discovery root: a documentation
    // directory such as docs/process/build/ is content, not output.
    "target/",
    "dist/",
    "build/",
    "output/",
    "vendor/",
    "venv/",
    "**/.venv/",
    "**/site-packages/",
];

/// The compiled default exclusions.
#[derive(Clone, Debug)]
pub(crate) struct DefaultExclusions(Vec<SourcePattern>); // bounded: PATTERNS.

impl DefaultExclusions {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self(
            PATTERNS
                .iter()
                .map(|value| SourcePattern::parse(value))
                .collect::<Result<_>>()?,
        ))
    }

    pub(crate) fn excludes(&self, relative: &str, kind: EntryKind) -> bool {
        self.0.iter().any(|pattern| pattern.matches(relative, kind))
    }
}
