//! Compile the bundled linked-data grammars once, never when publishing a
//! document.
#[path = "src/build_identity.rs"]
mod build_identity;
use {
    std::{env, path::PathBuf, process::Command},
    syntect::parsing::{SyntaxDefinition, SyntaxSet},
};

fn main() {
    identity();
    let mut builder = SyntaxSet::new().into_builder();
    for grammar in [
        VendoredGrammar(
            "src/publication/render/syntaxes/sparql.sublime-syntax",
        ),
        VendoredGrammar(
            "src/publication/render/syntaxes/turtle.sublime-syntax",
        ),
    ] {
        println!("cargo:rerun-if-changed={}", grammar.0);
        builder.add(grammar.definition());
    }
    let destination =
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
            .join("linked-data.packdump");
    syntect::dumps::dump_to_file(&builder.build(), destination)
        .expect("write compiled SPARQL grammar");
}

/// A Sublime grammar vendored unchanged from an upstream package.
struct VendoredGrammar(&'static str);

impl VendoredGrammar {
    fn definition(&self) -> SyntaxDefinition {
        let source = std::fs::read_to_string(self.0)
            .expect("read bundled linked-data grammar");
        SyntaxDefinition::load_from_str(
            &Self::syntect_source(&source),
            false,
            None,
        )
        .expect("compile bundled linked-data grammar")
    }

    /// Sublime Text accepts quoted capture-group keys (`'1': scope`), but
    /// Syntect only reads integer keys and silently drops the others. The
    /// LinkedData grammars quote every key, which removed every captured
    /// scope: prefixed-name namespaces, numbers, base and prefix IRIs.
    /// Unquote them so the compiled grammar keeps the upstream scopes.
    fn syntect_source(source: &str) -> String {
        let mut output = String::with_capacity(source.len());
        for line in source.split_inclusive('\n') {
            let body = line.trim_start();
            let indent = &line[..line.len() - body.len()];
            let key = body
                .strip_prefix('\'')
                .and_then(|rest| rest.split_once("':"))
                .filter(|(digits, _)| {
                    !digits.is_empty() &&
                        digits.bytes().all(|byte| byte.is_ascii_digit())
                });
            match key {
                Some((digits, rest)) => {
                    output.push_str(indent);
                    output.push_str(digits);
                    output.push(':');
                    output.push_str(rest);
                },
                None => output.push_str(line),
            }
        }
        output
    }
}

fn identity() {
    for name in [
        "GRAPHARCH_BUILD_REVISION",
        "GRAPHARCH_BUILD_RELEASE_VERSION",
        "GRAPHARCH_BUILD_CONTEXT",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for path in [
        "src",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "docs/other/ontology",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let explicit = env::var("GRAPHARCH_BUILD_REVISION").ok();
    let git = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"),
    )
    .join(".git")
    .exists();
    let external = explicit.is_some();
    let mut revision = explicit;
    let mut dirty = false;
    if revision.is_none() && git {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("read generator Git revision");
        assert!(
            output.status.success(),
            "cannot read generator Git revision"
        );
        revision = Some(
            String::from_utf8(output.stdout)
                .expect("Git revision UTF-8")
                .trim()
                .to_owned(),
        );
        let status = Command::new("git")
            .args(["diff-index", "--quiet", "HEAD", "--"])
            .status()
            .expect("inspect generator source state");
        match status.code() {
            Some(0) => {},
            Some(1) => dirty = true,
            _ => panic!("cannot inspect generator source state"),
        };
    }
    if git && !external {
        for name in ["HEAD", "index"] {
            let output = Command::new("git")
                .args(["rev-parse", "--git-path", name])
                .output()
                .expect("locate Git identity dependency");
            assert!(
                output.status.success(),
                "cannot locate Git identity dependency"
            );
            println!(
                "cargo:rerun-if-changed={}",
                String::from_utf8(output.stdout).unwrap().trim()
            );
        }
        let output = Command::new("git")
            .args(["symbolic-ref", "--quiet", "HEAD"])
            .output()
            .expect("inspect Git HEAD");
        if output.status.success() {
            let name = String::from_utf8(output.stdout).unwrap();
            let output = Command::new("git")
                .args(["rev-parse", "--git-path", name.trim()])
                .output()
                .expect("locate generator ref");
            assert!(
                output.status.success(),
                "cannot locate generator ref"
            );
            println!(
                "cargo:rerun-if-changed={}",
                String::from_utf8(output.stdout).unwrap().trim()
            );
        } else {
            assert_eq!(
                output.status.code(),
                Some(1),
                "cannot inspect Git HEAD"
            );
        }
    }
    let context = env::var("GRAPHARCH_BUILD_CONTEXT")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| {
            assert!(
                value.len() <= 4096,
                "generator build context exceeds byte budget"
            );
            serde_json::from_str::<build_identity::CiBuild>(&value)
                .expect("typed generator build context")
        });
    let value = build_identity::Identity::new(
        &env::var("CARGO_PKG_VERSION").unwrap(),
        revision.as_deref(),
        dirty,
        env::var("GRAPHARCH_BUILD_RELEASE_VERSION")
            .ok()
            .filter(|v| !v.is_empty())
            .as_deref(),
        context,
        external,
    )
    .expect("valid generator build identity");
    println!(
        "cargo:rustc-env=GRAPHARCH_BUILD_LABEL={}",
        value.label
    );
    println!(
        "cargo:rustc-env=GRAPHARCH_BUILD_CHANNEL={}",
        value.channel
    );
    println!(
        "cargo:rustc-env=GRAPHARCH_GENERATOR_REVISION={}",
        value.revision
    );
    println!(
        "cargo:rustc-env=GRAPHARCH_BUILD_NUMBER={}",
        value.number
    );
    println!(
        "cargo:rustc-env=GRAPHARCH_BUILD_ATTEMPT={}",
        value.attempt
    );
    println!(
        "cargo:rustc-env=GRAPHARCH_BUILD_RUN_URL={}",
        value.run_url
    );
    println!(
        "cargo:rustc-env=GRAPHARCH_BUILD_DETAILS={}",
        value.details
    );
    println!("cargo:rustc-env=GRAPHARCH_GENERATOR_MODIFIED={dirty}");
}
