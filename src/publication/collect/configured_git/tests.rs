use {
    super::*,
    crate::publication::{
        collect::{RdfFile, git::remote_documentation},
        dataset::Dataset,
    },
};

const PROJECT: &str = "urn:test:project";
const DEFINITIONS: &str = "urn:test:definitions";
const PREFIXES: &str = "@prefix ga: <https://ekgf.org/ontology/grapharch#> .\n\
                        @prefix dct: <http://purl.org/dc/terms/> .\n\
                        @prefix t: <urn:test:> .\n";

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

/// A content tree that carries no GraphArch configuration.
fn content() -> Result<(tempfile::TempDir, String, String)> {
    let root = tempfile::tempdir()?;
    git(root.path(), &["init", "-b", "main"])?;
    std::fs::create_dir(root.path().join("docs"))?;
    for (path, body) in [
        (
            "ontology.ttl",
            "<urn:test:Ontology> a <http://www.w3.org/2002/07/owl#Ontology> \
             .\n",
        ),
        (
            "docs/page.md",
            "# Page\n\nPAGE BODY\n\n![Badge](badge.svg)\n",
        ),
        (
            "docs/badge.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" \
             height=\"2\"/>",
        ),
        ("docs/notes.md", "# Notes\n\nNOTES BODY\n"),
        ("example.json", "{\"example\": \"CODE BODY\"}"),
        (
            "template.html",
            "<script>ignored()</script><details \
             class=\"preamble-section\"><summary>Preamble</summary><p>LEGAL \
             BODY</p></details>",
        ),
    ] {
        std::fs::write(root.path().join(path), body)?;
    }
    git(root.path(), &["add", "."])?;
    git(root.path(), &["commit", "-m", "content"])?;
    let commit = git(root.path(), &["rev-parse", "HEAD"])?;
    let url = url::Url::from_directory_path(root.path())
        .unwrap()
        .to_string();
    Ok((root, url, commit))
}

async fn run(definitions: &str) -> Result<Dataset> {
    let (_root, url, commit) = content()?;
    let cache = tempfile::tempdir()?;
    let tree =
        remote_documentation(&url, &commit, cache.path(), PROJECT).await?;
    let file = tempfile::NamedTempFile::with_suffix(".ttl")?;
    std::fs::write(file.path(), format!("{PREFIXES}{definitions}"))?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = crate::publication::writer::GraphWriter::spawn(
        dataset.clone(),
        8,
        4 * 1024 * 1024,
    )?;
    let result = async {
        RdfFile::new(PROJECT, DEFINITIONS, file.path().to_owned())?
            .collect(&writer)
            .await?;
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for_processor(PROJECT, DEFINITIONS).await?,
        );
        collect(
            &tree,
            &view,
            &NamedNode::new(PROJECT)?,
            &writer,
            4,
        )
        .await
    }
    .await;
    drop(writer);
    worker.await??;
    result?;
    Ok(dataset)
}

async fn count(dataset: &Dataset, pattern: &str) -> Result<String> {
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for(PROJECT).await?,
    );
    let pattern = view.pattern(pattern, 0);
    let mut rows = dataset
        .select(format!(
            "SELECT (COUNT(*) AS ?count) WHERE {{ {pattern} }}"
        ))
        .await?;
    let row = rows.next().await?.context("count row")?;
    let Term::Literal(count) = &row["count"] else {
        bail!("count literal required")
    };
    Ok(count.value().to_owned())
}

const USES: &str = r#"
t:project ga:collector t:ontology, t:page, t:notes, t:code, t:legal .
t:scope a ga:Scope .
t:ontology a ga:CollectorUse ; ga:collectorType ga:RdfFileCollector ;
    ga:source "ontology.ttl" ; ga:sourcePath "ontology.ttl" ; ga:subject t:Ontology .
t:page a ga:CollectorUse ; ga:collectorType ga:MarkdownPageCollector ;
    ga:source "docs/page.md" ; ga:sourcePath "docs/page.md" ; ga:fragment t:page-fragment ;
    ga:includedIn t:scope ; dct:title "Page" ; ga:route "page/" .
t:notes a ga:CollectorUse ; ga:collectorType ga:MarkdownFileCollector ;
    ga:source "docs/notes.md" ; ga:sourcePath "docs/notes.md" ; ga:fragment t:notes-fragment ;
    ga:includedIn t:scope .
t:code a ga:CollectorUse ; ga:collectorType ga:CodeFileCollector ;
    ga:source "example.json" ; ga:sourcePath "example.json" ; ga:format "application/json" ;
    ga:subject t:example ; ga:fragment t:code-fragment ; ga:includedIn t:scope .
t:legal a ga:CollectorUse ; ga:collectorType ga:HtmlPreambleCollector ;
    ga:source "template.html" ; ga:sourcePath "template.html" ; ga:fragment t:legal-preamble ;
    ga:includedIn t:scope .
"#;

#[tokio::test]
async fn configured_uses_build_existing_collectors_over_the_pinned_tree()
-> Result<()> {
    let dataset = run(USES).await?;
    for (pattern, expected) in [
        (
            "<urn:test:Ontology> a <http://www.w3.org/2002/07/owl#Ontology>",
            "1",
        ),
        (
            "?asset a <https://ekgf.org/ontology/grapharch#BinaryAsset>",
            "1",
        ),
        (
            "<urn:test:legal-preamble> \
             <https://ekgf.org/ontology/grapharch#format> 'text/html'",
            "1",
        ),
        (
            "?target <https://ekgf.org/ontology/grapharch#preamble> ?fragment",
            "0",
        ),
    ] {
        assert_eq!(count(&dataset, pattern).await?, expected, "{pattern}");
    }
    for (fragment, text) in [
        ("urn:test:notes-fragment", "NOTES BODY"),
        ("urn:test:code-fragment", "CODE BODY"),
        ("urn:test:legal-preamble", "LEGAL BODY"),
    ] {
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for(PROJECT).await?,
        );
        let body = view.text(&NamedNode::new(fragment)?, "body").await?;
        assert!(body.contains(text), "{fragment}: {body}");
        assert!(!body.contains("ignored()"));
    }
    Ok(())
}

#[tokio::test]
async fn invalid_uses_name_their_collector_use() -> Result<()> {
    for (definitions, expected) in [
        (
            r#"t:project ga:collector t:a, t:b . t:scope a ga:Scope .
t:a a ga:CollectorUse ; ga:collectorType ga:MarkdownFileCollector ; ga:source "docs/notes.md" ; ga:sourcePath "docs/notes.md" ; ga:fragment t:same ; ga:includedIn t:scope .
t:b a ga:CollectorUse ; ga:collectorType ga:MarkdownFileCollector ; ga:source "docs/page.md" ; ga:sourcePath "docs/page.md" ; ga:fragment t:same ; ga:includedIn t:scope ."#,
            "duplicate authored fragment <urn:test:same>",
        ),
        (
            r#"t:project ga:collector t:query .
t:query a ga:CollectorUse ; ga:collectorType ga:SparqlFileCollector ; ga:source "ontology.ttl" ; ga:sourcePath "ontology.ttl" ."#,
            "configured collector use <urn:test:query>",
        ),
        (
            r#"t:project ga:collector t:page . t:scope a ga:Scope .
t:page a ga:CollectorUse ; ga:collectorType ga:MarkdownPageCollector ; ga:source "docs/page.md" ; ga:sourcePath "docs/page.md" ; ga:fragment t:page-fragment ; ga:includedIn t:scope ; ga:route "page/" ."#,
            "page collector requires dct:title",
        ),
        (
            r#"t:project ga:collector t:missing .
t:missing a ga:CollectorUse ; ga:collectorType ga:RdfFileCollector ; ga:source "absent.ttl" ; ga:sourcePath "absent.ttl" ."#,
            "missing required Git blob absent.ttl",
        ),
        (
            r#"t:project ga:collector t:untyped .
t:untyped ga:collectorType ga:RdfFileCollector ; ga:source "ontology.ttl" ; ga:sourcePath "ontology.ttl" ."#,
            "collector must be typed ga:CollectorUse",
        ),
    ] {
        let error = match run(definitions).await {
            Ok(_) => bail!("accepted {definitions}"),
            Err(error) => format!("{error:#}"),
        };
        assert!(error.contains(expected), "{expected}: {error}");
    }
    Ok(())
}

/// A fragment outside every scope is collected but never published, so a
/// use that contributes a fragment must name the scopes it is included in.
#[tokio::test]
async fn fragment_contributing_uses_require_a_scope_inclusion() -> Result<()> {
    for (kind, settings) in [
        (
            "MarkdownFileCollector",
            r#"ga:source "docs/notes.md" ; ga:sourcePath "docs/notes.md""#,
        ),
        (
            "MarkdownPageCollector",
            r#"ga:source "docs/page.md" ; ga:sourcePath "docs/page.md" ; dct:title "Page" ; ga:route "page/""#,
        ),
        (
            "CodeFileCollector",
            r#"ga:source "example.json" ; ga:sourcePath "example.json" ; ga:format "application/json" ; ga:subject t:example"#,
        ),
        (
            "HtmlPreambleCollector",
            r#"ga:source "template.html" ; ga:sourcePath "template.html""#,
        ),
    ] {
        let definitions = format!(
            "t:project ga:collector t:use .\nt:use a ga:CollectorUse ; \
             ga:collectorType ga:{kind} ; {settings} ; ga:fragment t:fragment \
             ."
        );
        let error = match run(&definitions).await {
            Ok(_) => bail!("accepted {kind} without ga:includedIn"),
            Err(error) => format!("{error:#}"),
        };
        assert!(
            error.contains(
                "<urn:test:use> contributes fragment <urn:test:fragment> but \
                 declares no ga:includedIn"
            ),
            "{kind}: {error}"
        );
    }
    let error = match run("t:project ga:collector t:use .\nt:use a \
                           ga:CollectorUse ; ga:collectorType \
                           ga:MarkdownFileCollector ; ga:source \
                           \"docs/notes.md\" ; ga:sourcePath \
                           \"docs/notes.md\" ; ga:fragment t:fragment ; \
                           ga:includedIn t:undeclared .")
    .await
    {
        Ok(_) => bail!("accepted an undeclared scope"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        error.contains("references undefined ga:Scope"),
        "{error}"
    );
    Ok(())
}

#[tokio::test]
async fn collector_budget_is_enforced_before_any_source_is_read() -> Result<()>
{
    let mut definitions = String::new();
    for index in 0..=COLLECTOR_USES {
        definitions.push_str(&format!(
            "t:project ga:collector t:use{index} .\n"
        ));
    }
    let error = match run(&definitions).await {
        Ok(_) => bail!("budget not enforced"),
        Err(error) => format!("{error:#}"),
    };
    assert!(error.contains("64-collector budget"), "{error}");
    Ok(())
}
