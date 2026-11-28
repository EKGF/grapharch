//! Package manifests, parsed with typed parsers and never executed.
mod cargo;
mod go_mod;
mod package_json;
mod pyproject;
mod scan;

pub(crate) use scan::Manifests;

#[cfg(test)]
mod tests;
