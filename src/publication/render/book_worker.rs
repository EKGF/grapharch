//! Bounded native child processes compile prepared Typst book jobs. They never
//! open the coordinator's store, collect sources, or mutate publication graphs.
use {
    super::engine,
    anyhow::{Result, ensure},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
    },
    tokio::{io::AsyncReadExt, process::Command},
};
pub(super) fn executable(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_owned());
    }
    let path = std::env::current_exe()?;
    ensure!(
        path.file_stem().and_then(|s| s.to_str()) == Some("grapharch"),
        "library bookshelf publishing requires \
         with_book_workers(grapharch_binary, jobs)"
    );
    Ok(path)
}
pub(super) async fn run(
    executable: &Path,
    root: &Path,
    main: &str,
    expected_engine: &str,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    let mut child = Command::new(executable)
        .args(["publication", "compile-book", "--root"])
        .arg(root)
        .arg("--main")
        .arg(main)
        .arg("--expected-engine")
        .arg(expected_engine)
        .arg("--watch-parent")
        .env("TOKIO_WORKER_THREADS", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("missing worker error pipe"))?;
    let mut message = Vec::new();
    let mut stderr = stderr.take(64 * 1024 + 1);
    let interrupted = tokio::select! {
        result=stderr.read_to_end(&mut message)=>{result?;None},
        _=cancel.changed()=>Some("book compilation cancelled"),
        _=tokio::time::sleep(std::time::Duration::from_secs(1800))=>Some("book worker exceeded the 30-minute deadline"),
    };
    if let Some(reason) = interrupted {
        if child.try_wait()?.is_none() {
            child.kill().await?;
        }
        anyhow::bail!("{reason}");
    }
    ensure!(
        message.len() <= 64 * 1024,
        "book worker exceeded diagnostic byte budget"
    );
    let status = child.wait().await?;
    ensure!(
        status.success(),
        "book worker {main} exited with {status}: {}",
        String::from_utf8_lossy(&message)
    );
    let path = root.join(main).with_extension("pdf");
    let mut file = tokio::fs::File::open(path).await?;
    let mut magic = [0; 5];
    tokio::io::AsyncReadExt::read_exact(&mut file, &mut magic).await?;
    ensure!(
        &magic == b"%PDF-",
        "book worker did not produce a PDF"
    );
    Ok(())
}
pub(super) async fn compile(root: &Path, main: &str) -> Result<()> {
    ensure!(
        main.strip_prefix("book-")
            .and_then(|s| s.strip_suffix(".typ"))
            .is_some_and(|s| {
                s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
            }),
        "invalid book worker source filename"
    );
    let root = tokio::fs::canonicalize(root).await?;
    let source = root.join(main);
    let meta = tokio::fs::symlink_metadata(&source).await?;
    ensure!(
        meta.is_file() &&
            !meta.file_type().is_symlink() &&
            meta.len() <= crate::publication::limits::TYPST_SOURCE_BYTES,
        "invalid or oversized book source"
    );
    let assets = engine::Assets::load().await?;
    let pdf = engine::compile_file(
        root.clone(),
        main.into(),
        assets,
        Arc::new(tokio::sync::Semaphore::new(1)),
    )
    .await?;
    ensure!(
        pdf.len() as u64 <= crate::publication::limits::ARTIFACT_BYTES,
        "book PDF exceeds byte budget"
    );
    tokio::fs::write(source.with_extension("pdf"), pdf).await?;
    Ok(())
}
