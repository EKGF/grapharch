use {
    super::{CHUNK_BYTES, PublishedOutput},
    anyhow::Result,
    std::{collections::BTreeSet, path::Path},
};

fn output(
    root: &Path,
    pages: &[(&str, &str)],
    others: &[&str],
) -> Result<PublishedOutput> {
    let mut files = BTreeSet::new();
    for (path, body) in pages
        .iter()
        .map(|(path, body)| (*path, *body))
        .chain(others.iter().map(|path| (*path, "%PDF-1.7")))
    {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap())?;
        std::fs::write(file, body)?;
        files.insert(path.to_owned());
    }
    Ok(PublishedOutput::new(root.to_owned(), files))
}

#[tokio::test]
async fn relative_links_to_published_files_pass() -> Result<()> {
    let root = tempfile::tempdir()?;
    output(
        root.path(),
        &[
            (
                "specification/index.html",
                "<!doctype html><link rel=\"stylesheet\" \
                 href=\"style.css\"><link rel=\"icon\" href=\"data:,\"><a \
                 href=\"../pdf/ekgf/document.pdf#page=2\">EKGF</a><a \
                 href=\"../reader/\">Reader</a><a href=\"#intro\">Intro</a><a \
                 href=\"https://ekgf.org/missing.pdf\">External</a><a \
                 href=\"mailto:info@example.org\">Mail</a><script>const \
                 link = '<a href=\"missing.html\">';</script><pre>&lt;a \
                 href=\"missing.pdf\"&gt;</pre><textarea><img \
                 src=\"missing.png\"></textarea>",
            ),
            ("reader/index.html", "<img src=\"../specification/style.css\">"),
        ],
        &["specification/style.css", "pdf/ekgf/document.pdf"],
    )?
    .verify_links()
    .await
}

#[tokio::test]
async fn unresolved_relative_links_fail_with_their_page_and_value() -> Result<()>
{
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("unpublished.pdf"), "%PDF-1.7")?;
    let error = output(
        root.path(),
        &[(
            "specification/index.html",
            "<a href=\"../pdf/omg/document.pdf\">Missing</a><a \
             href=\"../unpublished.pdf\">On disk only</a><a \
             href=\"../../outside.pdf\">Outside</a><a \
             href=\"/pdf/ekgf/document.pdf\">Host</a>",
        )],
        &["pdf/ekgf/document.pdf"],
    )?
    .verify_links()
    .await
    .expect_err("unresolved links must fail the showcase");
    let message = format!("{error:#}");
    assert!(
        message.starts_with("4 relative links"),
        "{message}"
    );
    for expected in [
        "specification/index.html: ../pdf/omg/document.pdf \
         (pdf/omg/document.pdf is not published by this showcase)",
        "../unpublished.pdf (unpublished.pdf is not published",
        "../../outside.pdf (it leaves the showcase output)",
        "/pdf/ekgf/document.pdf (it depends on the host's mount point)",
    ] {
        assert!(
            message.contains(expected),
            "{expected}: {message}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn links_after_split_characters_and_chunk_boundaries_are_checked()
-> Result<()> {
    let root = tempfile::tempdir()?;
    // A two-byte character straddles the first chunk boundary.
    let mut page = "é".repeat(CHUNK_BYTES / 2 - 1);
    page.push('a');
    page.push_str("ü<a href=\"missing.pdf\">x</a>");
    page.push_str(&"ü".repeat(CHUNK_BYTES));
    let error = output(root.path(), &[("index.html", &page)], &[])?
        .verify_links()
        .await
        .expect_err("a link split across chunks must still be checked");
    assert!(format!("{error:#}").contains("index.html: missing.pdf"));
    Ok(())
}

#[tokio::test]
async fn base_elements_and_invalid_utf8_are_rejected() -> Result<()> {
    let root = tempfile::tempdir()?;
    let error = output(
        root.path(),
        &[(
            "index.html",
            "<base href=\"https://ekgf.org/\"><a href=\"a.pdf\">",
        )],
        &["a.pdf"],
    )?
    .verify_links()
    .await
    .expect_err("a base element changes every relative link");
    assert!(format!("{error:#}").contains("declares <base>"));
    let root = tempfile::tempdir()?;
    let published = output(root.path(), &[("index.html", "")], &[])?;
    std::fs::write(root.path().join("index.html"), [b'<', 0xff, b'>'])?;
    assert!(published.verify_links().await.is_err());
    Ok(())
}
