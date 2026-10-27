#[tokio::test]
async fn book_worker_exits_when_its_coordinator_pipe_closes()
-> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let main = format!("book-{}.typ", "0".repeat(64));
    std::fs::write(root.path().join(&main), "#while true {}")?;
    let child = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "compile-book", "--root"])
        .arg(root.path())
        .arg("--main")
        .arg(main)
        .arg("--watch-parent")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        child.wait_with_output(),
    )
    .await??;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("coordinator disconnected"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
