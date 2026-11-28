//! The one repository-aware walker behind every automatic source discovery.
//!
//! Discovery honours the ignore files a repository already maintains, with or
//! without a `.git` directory, skips agent instructions, planning notes and
//! test fixtures by default, and stops at a typed, configurable source budget
//! instead of a fixed cap.
mod budget;
mod defaults;
mod walker;

pub(crate) use {budget::SourceBudget, walker::SourceWalk};

#[cfg(test)]
mod tests;
