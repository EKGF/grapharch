//! Compile the bundled SPARQL grammar once, never when publishing a document.
use {
    std::{env, path::PathBuf},
    syntect::parsing::{SyntaxDefinition, SyntaxSet},
};

fn main() {
    let grammar = "src/publication/render/syntaxes/sparql.sublime-syntax";
    println!("cargo:rerun-if-changed={grammar}");
    let mut builder = SyntaxSet::new().into_builder();
    builder.add(
        SyntaxDefinition::load_from_str(
            &std::fs::read_to_string(grammar)
                .expect("read bundled SPARQL grammar"),
            false,
            None,
        )
        .expect("compile bundled SPARQL grammar"),
    );
    let destination =
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
            .join("sparql.packdump");
    syntect::dumps::dump_to_file(&builder.build(), destination)
        .expect("write compiled SPARQL grammar");
}
