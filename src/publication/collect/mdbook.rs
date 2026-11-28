//! Bounded, non-executing import of source-owned mdBook instructions.
mod collector;
mod config;
mod navigation;
mod summary;
mod warning;

pub(crate) use collector::MdBook;
