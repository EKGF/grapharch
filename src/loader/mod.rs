mod r#impl;
mod this;

pub use {
    r#impl::{MarkdownLoader, RDFLoader},
    this::{Loader, LoaderImplementor},
};
