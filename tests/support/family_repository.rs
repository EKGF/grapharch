use {
    anyhow::{Result, ensure},
    std::path::Path,
};

pub fn git(path: &Path, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "Family fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.org")
        .env("GIT_COMMITTER_NAME", "Family fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.org")
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
        .output()?;
    ensure!(
        output.status.success(),
        "Git test fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn bare_copy(source: &Path, root: &Path) -> Result<(String, String)> {
    let checkout = root.join("checkout");
    copy_tree(source, &checkout)?;
    git(&checkout, &["init", "-b", "main"])?;
    git(&checkout, &["add", "--all"])?;
    git(&checkout, &[
        "commit",
        "-m",
        "Offline family fixture",
    ])?;
    let commit = git(&checkout, &["rev-parse", "HEAD"])?;
    let bare = root.join("remote.git");
    git(&checkout, &[
        "clone",
        "--bare",
        ".",
        bare.to_str().unwrap(),
    ])?;
    let url = url::Url::from_directory_path(bare).unwrap().to_string();
    Ok((url, commit))
}

pub fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}
