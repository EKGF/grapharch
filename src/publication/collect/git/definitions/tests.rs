use {
    super::*,
    crate::publication::{dataset::Dataset, writer::GraphWriter},
};

const PROJECT: &str = "urn:test:project";
const DEFINITIONS: &str =
    "<urn:test:project> a <https://ekgf.org/ontology/grapharch#Project> .\n";

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.org")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.org")
        .output()?;
    ensure!(
        output.status.success(),
        "fixture git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn controlling() -> Result<(tempfile::TempDir, PathBuf, String)> {
    let root = tempfile::tempdir()?;
    git(root.path(), &["init", "-b", "main"])?;
    std::fs::create_dir(root.path().join("publication"))?;
    let path = root.path().join("publication/definitions.ttl");
    std::fs::write(&path, DEFINITIONS)?;
    git(root.path(), &["add", "."])?;
    git(root.path(), &["commit", "-m", "definitions"])?;
    let commit = git(root.path(), &["rev-parse", "HEAD"])?;
    Ok((root, path, commit))
}

async fn rejection(path: &Path, commit: &str) -> String {
    match CommittedDefinitions::checkout(path, commit, PROJECT).await {
        Ok(_) => panic!("unverified definitions were accepted"),
        Err(error) => format!("{error:#}"),
    }
}

#[tokio::test]
async fn verified_checkout_definitions_come_from_the_committed_blob()
-> Result<()> {
    let (_root, path, commit) = controlling()?;
    let definitions =
        CommittedDefinitions::checkout(&path, &commit, PROJECT).await?;
    assert_eq!(definitions.commit(), commit);
    assert_eq!(definitions.path(), "publication/definitions.ttl");
    assert_eq!(
        definitions.blob().bytes(MAX_RDF_BYTES).await?,
        DEFINITIONS.as_bytes()
    );
    assert_eq!(
        definitions.sha256(),
        hex::encode(Sha256::digest(DEFINITIONS.as_bytes()))
    );
    assert_eq!(definitions.blob_id().len(), 40);
    assert_eq!(
        definitions.provenance().identity()?.processor,
        DEFINITIONS_PROVENANCE
    );
    assert!(definitions.repository().starts_with("file://"));
    Ok(())
}

#[tokio::test]
async fn dirty_untracked_staged_linked_and_mismatched_definitions_are_rejected()
-> Result<()> {
    let (root, path, commit) = controlling()?;
    std::fs::write(&path, format!("{DEFINITIONS}# local edit\n"))?;
    assert!(
        rejection(&path, &commit)
            .await
            .contains("definitions differ from committed controlling revision")
    );
    git(root.path(), &["add", "."])?;
    std::fs::write(&path, DEFINITIONS)?;
    assert!(
        rejection(&path, &commit)
            .await
            .contains("definitions differ from committed controlling revision"),
        "a staged but uncommitted change is not the controlling revision"
    );
    git(root.path(), &["reset", "--hard"])?;
    let untracked = root.path().join("publication/untracked.ttl");
    std::fs::write(&untracked, DEFINITIONS)?;
    assert!(
        rejection(&untracked, &commit)
            .await
            .contains("not committed")
    );
    #[cfg(unix)]
    {
        let link = root.path().join("publication/link.ttl");
        std::os::unix::fs::symlink(&path, &link)?;
        assert!(rejection(&link, &commit).await.contains("regular file"));
        std::fs::remove_file(link)?;
    }
    std::fs::remove_file(untracked)?;
    std::fs::write(root.path().join("other.txt"), "next\n")?;
    git(root.path(), &["add", "."])?;
    git(root.path(), &["commit", "-m", "advance"])?;
    assert!(
        rejection(&path, &commit)
            .await
            .contains("differs from definitions commit"),
        "HEAD must be exactly the declared controlling commit"
    );
    Ok(())
}

#[tokio::test]
async fn content_and_definitions_from_one_origin_keep_separate_provenance()
-> Result<()> {
    let (root, _path, first) = controlling()?;
    std::fs::write(root.path().join("content.md"), "# Content\n")?;
    git(root.path(), &["add", "."])?;
    git(root.path(), &["commit", "-m", "content"])?;
    let second = git(root.path(), &["rev-parse", "HEAD"])?;
    let cache = tempfile::tempdir()?;
    let url = url::Url::from_directory_path(root.path())
        .unwrap()
        .to_string();
    let content =
        remote_documentation(&url, &second, cache.path(), PROJECT).await?;
    let controlling =
        remote_documentation(&url, &first, cache.path(), PROJECT).await?;
    let definitions = CommittedDefinitions::fetched(
        &controlling,
        Path::new("publication/definitions.ttl"),
    )
    .await?;
    assert_eq!(definitions.commit(), first);
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
    content.provenance.collect(&writer).await?;
    definitions.provenance().collect(&writer).await?;
    drop(writer);
    worker.await??;
    let view = crate::publication::view::RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(PROJECT).await?,
    );
    let pattern = view.pattern(
        "?repository <http://www.w3.org/ns/prov#hadMember> ?commit",
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT (COUNT(DISTINCT ?commit) AS ?count) WHERE {{ {pattern} }}"
        ))
        .await?;
    let row = rows.next().await?.context("count row")?;
    let Term::Literal(count) = &row["count"] else {
        anyhow::bail!("count literal required")
    };
    assert_eq!(
        count.value(),
        "2",
        "definitions provenance replaced content provenance"
    );
    Ok(())
}
