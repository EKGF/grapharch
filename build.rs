//! Compile the bundled linked-data grammars once, never when publishing a
//! document.
use {
    std::{env, path::PathBuf},
    syntect::parsing::{SyntaxDefinition, SyntaxSet},
};

fn main() {
    let mut builder = SyntaxSet::new().into_builder();
    for grammar in [
        "src/publication/render/syntaxes/sparql.sublime-syntax",
        "src/publication/render/syntaxes/turtle.sublime-syntax",
    ] {
        println!("cargo:rerun-if-changed={grammar}");
        builder.add(
            SyntaxDefinition::load_from_str(
                &std::fs::read_to_string(grammar)
                    .expect("read bundled linked-data grammar"),
                false,
                None,
            )
            .expect("compile bundled linked-data grammar"),
        );
    }
    let destination =
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
            .join("linked-data.packdump");
    syntect::dumps::dump_to_file(&builder.build(), destination)
        .expect("write compiled SPARQL grammar");
}
