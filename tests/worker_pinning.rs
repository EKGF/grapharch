use {
    anyhow::Result,
    std::path::{Path, PathBuf},
    tokio::process::Command,
};

fn worker_path(root: &Path) -> PathBuf {
    root.join(format!(
        ".grapharch-book-worker{}",
        std::env::consts::EXE_SUFFIX
    ))
}

#[tokio::test]
async fn worker_image_is_prepared_by_an_exited_child_and_can_be_executed()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let output = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "prepare-book-worker", "--root"])
        .arg(root.path())
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let original = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("--version")
        .output()
        .await?;
    let pinned = Command::new(worker_path(root.path()))
        .arg("--version")
        .output()
        .await?;
    assert!(
        pinned.status.success(),
        "{}",
        String::from_utf8_lossy(&pinned.stderr)
    );
    assert_eq!(pinned.stdout, original.stdout);
    assert_eq!(
        tokio::fs::metadata(worker_path(root.path())).await?.len(),
        tokio::fs::metadata(env!("CARGO_BIN_EXE_grapharch"))
            .await?
            .len()
    );
    Ok(())
}

#[tokio::test]
async fn worker_preparation_rejects_engine_mismatch_before_writing()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let output = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "prepare-book-worker", "--root"])
        .arg(root.path())
        .args(["--expected-engine", "wrong-engine"])
        .output()
        .await?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("compiler fingerprint mismatch")
    );
    assert!(!tokio::fs::try_exists(worker_path(root.path())).await?);
    Ok(())
}
