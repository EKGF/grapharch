//! A credential-free, local-only, time- and output-bounded PlantUML process.
use {
    anyhow::{Context, Result, ensure},
    std::{ffi::OsStr, process::Stdio, time::Duration},
    tokio::{
        io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
        process::Command,
    },
};
async fn read(
    mut stream: impl AsyncRead + Unpin,
    limit: u64,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new(); // bounded: each process pipe has an explicit cap plus overflow byte.
    (&mut stream)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(
        bytes.len() as u64 <= limit,
        "PlantUML process output exceeds byte budget"
    );
    Ok(bytes)
}
fn launcher(program: &OsStr, windows: bool) -> &OsStr {
    // Rust only supplies the .exe suffix during Windows PATH lookup. Our
    // explicitly provisioned Java launcher is a .cmd file on that platform.
    if windows && program == OsStr::new("plantuml") {
        OsStr::new("plantuml.cmd")
    } else {
        program
    }
}
pub(super) fn command(program: &OsStr, args: &[&str]) -> Command {
    let mut command = Command::new(launcher(program, cfg!(windows)));
    command
        .args(args)
        .env_clear()
        .env("PLANTUML_SECURITY_PROFILE", "SANDBOX")
        .env("PLANTUML_LIMIT_SIZE", "8192")
        .env("LANG", "C.UTF-8")
        .env("TZ", "UTC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for key in ["PATH", "JAVA_HOME", "SYSTEMROOT"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
}
pub(super) async fn run(
    program: &OsStr,
    args: &[&str],
    source: &[u8],
    limit: u64,
) -> Result<Vec<u8>> {
    let mut command = command(program, args);
    let mut child = command.spawn().context(
        "PlantUML diagrams require a local PlantUML installation (and \
         Graphviz for graph layouts) on PATH; GraphArch never downloads a \
         renderer",
    )?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    tokio::time::timeout(Duration::from_secs(30), async move {
        let write = async move {
            stdin.write_all(source).await?;
            stdin.shutdown().await?;
            drop(stdin);
            anyhow::Ok(())
        };
        let ((), output, error) = tokio::try_join!(
            write,
            read(stdout, limit),
            read(stderr, 64 * 1024)
        )?;
        let status = child.wait().await?;
        ensure!(
            status.success(),
            "PlantUML failed ({status}): {}",
            String::from_utf8_lossy(&error)
        );
        Ok(output)
    })
    .await
    .context("PlantUML exceeded its 30-second rendering budget")?
}
pub(super) async fn version(program: &OsStr) -> Result<String> {
    let bytes = run(program, &["-version"], b"", 64 * 1024).await?;
    let text = std::str::from_utf8(&bytes)?;
    let first = text.lines().next().unwrap_or("").trim();
    ensure!(
        first.starts_with("PlantUML version ") && first.len() <= 512,
        "renderer must identify itself as PlantUML"
    );
    Ok(first.into())
}
pub(super) fn clean_svg(bytes: &[u8]) -> Result<Vec<u8>> {
    let source = std::str::from_utf8(bytes)?;
    let mut result = String::new();
    let mut offset = 0;
    let mut metadata = 0;
    for token in xmlparser::Tokenizer::from(source) {
        if let xmlparser::Token::ProcessingInstruction {
            target,
            content,
            span,
        } = token?
        {
            metadata += 1;
            ensure!(
                metadata == 1 &&
                    target.as_str() == "plantuml" &&
                    content.is_some_and(|value| {
                        value.as_str().len() <= 64 &&
                            value.as_str().bytes().all(|b| {
                                b.is_ascii_alphanumeric() ||
                                    matches!(b, b'.' | b'-')
                            })
                    }),
                "unsupported generated SVG processing instruction"
            );
            // Tool provenance is retained explicitly in the KG; no active PI is
            // published.
            result.push_str(&source[offset..span.start()]);
            offset = span.end();
        }
    }
    result.push_str(&source[offset..]);
    Ok(result.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_default_uses_the_explicit_batch_launcher_extension() {
        assert_eq!(
            launcher(OsStr::new("plantuml"), true),
            "plantuml.cmd"
        );
        assert_eq!(
            launcher(OsStr::new("plantuml"), false),
            "plantuml"
        );
        assert_eq!(
            launcher(OsStr::new("custom.exe"), true),
            "custom.exe"
        );
    }
}
