mod console;
mod this;
mod typst;

pub use {
    console::ConsoleGenerator,
    this::{DocumentationGenerator, Generator},
    typst::TypstGenerator,
};
