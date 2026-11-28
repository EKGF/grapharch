//! Forge metadata behind one trait, with one implementation per forge.
mod cache;
pub(super) mod client;
mod github;
mod gitlab;
mod metadata;

pub(crate) use {
    cache::ForgeCache,
    client::{ForgeAccess, ForgeApi},
    metadata::{ForgeMetadata, ForgeRelease, Visibility},
};

#[cfg(test)]
mod tests;
