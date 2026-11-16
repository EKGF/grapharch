//! Bounded native child processes compile prepared Typst book jobs. They never
//! open the coordinator's store, collect sources, or mutate publication graphs.
use {
    super::{book_process::BookProcess, engine},
    anyhow::{Result, ensure},
    std::{
        ffi::OsStr,
        path::{Path, PathBuf},
        sync::Arc,
    },
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
fn image_path(root: &Path) -> PathBuf {
    root.join(format!(
        ".grapharch-book-worker{}",
        std::env::consts::EXE_SUFFIX
    ))
}

/// The coordinator never opens its pinned executable for writing. Otherwise a
/// concurrent fork can inherit that descriptor and make exec fail with ETXTBSY,
/// even after the coordinator closes its own copy (Rust issue 114554).
pub(super) async fn pin(
    executable: &Path,
    root: &Path,
    expected_engine: &str,
) -> Result<PathBuf> {
    BookProcess::spawn(executable, &[
        OsStr::new("prepare-book-worker"),
        OsStr::new("--root"),
        root.as_os_str(),
        OsStr::new("--expected-engine"),
        OsStr::new(expected_engine),
    ])?
    .finish(None)
    .await?
    .succeeded("book worker preparation")?;
    Ok(image_path(root))
}

pub(super) async fn prepare(root: &Path) -> Result<()> {
    let root = tokio::fs::canonicalize(root).await?;
    ensure!(
        tokio::fs::metadata(&root).await?.is_dir(),
        "worker preparation root must be a directory"
    );
    let executable = std::env::current_exe()?;
    let meta = tokio::fs::metadata(&executable).await?;
    ensure!(
        meta.is_file() && meta.len() <= 512 * 1024 * 1024,
        "worker executable exceeds byte budget"
    );
    let worker = image_path(&root);
    match tokio::fs::symlink_metadata(&worker).await {
        Ok(_) => anyhow::bail!("worker image already exists"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.into()),
    }
    tokio::fs::copy(executable, worker).await?;
    Ok(())
}

pub(super) async fn run(
    executable: &Path,
    root: &Path,
    main: &str,
    expected_engine: &str,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    BookProcess::spawn(executable, &[
        OsStr::new("compile-book"),
        OsStr::new("--root"),
        root.as_os_str(),
        OsStr::new("--main"),
        OsStr::new(main),
        OsStr::new("--expected-engine"),
        OsStr::new(expected_engine),
    ])?
    .finish(Some(cancel))
    .await?
    .succeeded(&format!("book worker {main}"))?;
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
