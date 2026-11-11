#[path = "support/dprod_header.rs"]
mod dprod_header;

use {
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
    std::path::Path,
};

#[tokio::test]
#[ignore = "explicit remote acceptance; performs read-only network collection"]
async fn remote_revision_restores_and_publishes_declared_targets()
-> anyhow::Result<()> {
    let cache = Path::new("output/cache/git");
    let config = std::env::var("GRAPHARCH_DPROD_CONFIG")
        .unwrap_or_else(|_| "examples/dprod/live.yaml".into());
    let (dataset, plans) =
        showcases::revision_build(Path::new(&config), cache).await?;
    let output = Path::new("output/dprod-editions-acceptance");
    tokio::fs::create_dir_all(output).await?;
    let archive = output.join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output).await?;
    assert_eq!(publications.len(), 5);
    for edition in ["ekgf", "omg"] {
        let directory = output.join("pdf").join(edition);
        assert!(
            tokio::fs::read(directory.join("document.pdf"))
                .await?
                .starts_with(b"%PDF-")
        );
    }
    let overview =
        tokio::fs::read_to_string(output.join("pdf/overview/main.typ")).await?;
    assert!(overview.contains("DPROD: the bigger picture"));
    assert!(overview.contains("A product in practice"));
    assert!(
        !overview.contains("RDF predicate"),
        "audience overview must exclude the technical catalogue"
    );
    let html =
        tokio::fs::read_to_string(output.join("specification/index.html"))
            .await?;
    dprod_header::assert_restored_metadata(
        &html,
        &output.join("specification"),
    )
    .await?;
    assert!(html.contains("Data Product"));
    for title in [
        "Conformance",
        "Normative references",
        "Normative namespaces",
        "Data Contracts",
        "Provider Duties",
        "Data Lineage",
    ] {
        assert!(
            html.contains(title),
            "missing formal content: {title}"
        );
    }
    assert!(
        !html.contains("Repository for the Data Products Ontology"),
        "marketing README must not become formal specification content"
    );
    let (writer, worker) = grapharch::publication::writer::GraphWriter::spawn(
        dataset.clone(),
        8,
        4 * 1024 * 1024,
    )?;
    let planner = grapharch::publication::plan::TargetPlanner::new(
        dataset.clone(),
        grapharch::publication::view::RevisionView::new(
            dataset.clone(),
            plans[0].snapshot.clone(),
        )
        .node(&plans[0].id, "inputRevision")
        .await?,
    );
    let failure = planner
        .resolve(
            "urn:dprod-showcase:project",
            &oxrdf::NamedNode::new("urn:dprod-showcase:release")?,
            &writer,
        )
        .await
        .expect_err("unapproved release must not be an active publication");
    assert!(format!("{failure:#}").contains("not registered"));
    drop(writer);
    worker.await??;
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> anyhow::Result<String> {
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
    anyhow::ensure!(
        output.status.success(),
        "fixture git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn configuration(
    path: &Path,
    remote: &Path,
    commit: &str,
    definitions: &str,
) -> anyhow::Result<()> {
    std::fs::write(
        path,
        format!(
            "repository: {}\ncommit: {commit}\ndefinitions:\n  commit: \
             {definitions}\n  path: \
             publication/definitions/develop.ttl\nproject: \
             urn:dprod-showcase:project\ntargets: all\n",
            remote.display()
        ),
    )?;
    Ok(())
}

async fn bodies(
    dataset: &Dataset,
    marker: &str,
) -> anyhow::Result<Vec<String>> {
    let mut rows = dataset
        .select(format!(
            "SELECT ?body WHERE {{ GRAPH ?g {{ ?f \
             <https://ekgf.org/ontology/grapharch#body> ?body }} \
             FILTER(CONTAINS(STR(?body), '{marker}')) }}"
        ))
        .await?;
    let mut bodies = Vec::new();
    while let Some(row) = rows.next().await? {
        let oxrdf::Term::Literal(body) = &row["body"] else {
            anyhow::bail!("source text required")
        };
        bodies.push(body.value().to_owned());
    }
    Ok(bodies)
}

/// Content commits on a designated branch, definitions on a controlling
/// branch of the same repository, all fetched by exact commit.
#[tokio::test]
async fn exact_revision_showcase_pins_content_and_definitions_without_fallback()
-> anyhow::Result<()> {
    let repo = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    git(repo.path(), &["init", "-b", "main"])?;
    std::fs::write(
        repo.path().join("README.md"),
        "# MAIN MUST NEVER APPEAR\n",
    )?;
    git(repo.path(), &["add", "."])?;
    git(repo.path(), &["commit", "-m", "main trap"])?;
    git(repo.path(), &[
        "checkout",
        "--orphan",
        "publication",
    ])?;
    git(repo.path(), &["rm", "-rf", "--quiet", "."])?;
    std::fs::create_dir_all(repo.path().join("publication/definitions"))?;
    std::fs::copy(
        "tests/fixtures/revision-showcase/definitions.ttl",
        repo.path().join("publication/definitions/develop.ttl"),
    )?;
    git(repo.path(), &["add", "."])?;
    git(repo.path(), &[
        "commit",
        "-m",
        "controlling definitions",
    ])?;
    let definitions = git(repo.path(), &["rev-parse", "HEAD"])?;
    git(repo.path(), &["checkout", "main"])?;
    git(repo.path(), &["checkout", "-b", "develop"])?;
    std::fs::create_dir_all(repo.path().join("publication"))?;
    std::fs::create_dir_all(repo.path().join("ontology/dprod"))?;
    for (source, target) in [
        (
            "content/ontology.ttl",
            "ontology/dprod/dprod-ontology.ttl",
        ),
        (
            "content/shapes.ttl",
            "ontology/dprod/dprod-shapes.ttl",
        ),
        ("content/abstract.md", "publication/abstract.md"),
        ("content/status.md", "publication/status.md"),
    ] {
        std::fs::copy(
            Path::new("examples/dprod").join(source),
            repo.path().join(target),
        )?;
    }
    let ontology_path = repo.path().join("ontology/dprod/dprod-ontology.ttl");
    let mut ontology = std::fs::read_to_string(&ontology_path)?;
    ontology.push_str(r#"
<https://www.omg.org/spec/DPROD/dprod/DataProduct> <http://www.w3.org/2004/02/skos/core#prefLabel> "Primary preferred label"@en, "Competing preferred label"@en ; <https://ekgf.org/ontology/annotation#markdownDescription> "Primary markdown explanation."@en, "Competing markdown explanation."@en .
<https://www.omg.org/spec/DPROD/dprod/DataProduct> <http://www.w3.org/2002/07/owl#oneOf> (<https://example.org/retained-enumeration-member>) .
"#);
    std::fs::write(ontology_path, ontology)?;
    std::fs::write(
        repo.path().join("README.md"),
        "# Development documentation\n\nDEVELOP ONE\n\n![Fixture \
         badge](badge.svg)\n\n[Download \
         badge](badge.svg)\n\n![PNG](tiny%20photo+%25.png)\n\n![JPEG](tiny.\
         jpg)\n",
    )?;
    std::fs::write(
        repo.path().join("badge.svg"),
        r##"<?xml version="1.0" encoding="UTF-8"?><svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#fff"/></svg>"##,
    )?;
    std::fs::write(
        repo.path().join("example.md"),
        "# Dataset example\n\nA dataset.\n\n![Shared badge](badge.svg)\n",
    )?;
    for (name, format) in [
        ("tiny photo+%.png", image::ImageFormat::Png),
        ("tiny.jpg", image::ImageFormat::Jpeg),
    ] {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2).write_to(&mut bytes, format)?;
        std::fs::write(repo.path().join(name), bytes.into_inner())?;
    }
    std::fs::write(
        repo.path().join("formal.md"),
        "# Formal scope\n\nSOURCE OWNED FORMAL SCOPE\n",
    )?;
    std::fs::write(
        repo.path().join("profile.ttl"),
        "<https://www.omg.org/spec/DPROD/dprod/> \
         <http://purl.org/dc/terms/title> \"RELATED PROFILE TITLE\" .",
    )?;
    std::fs::write(
        repo.path().join("example.jsonld"),
        "{\"title\":\"PINNED EXAMPLE SOURCE\"}",
    )?;
    std::fs::create_dir_all(repo.path().join("respec"))?;
    std::fs::write(
        repo.path().join("respec/template.html"),
        r#"<details class="preamble-section"><summary>Preamble</summary><p>Remote develop legal terms, retained in full.</p><p class="section-title">LICENSES</p><p>Permission subject to these notices.</p></details>"#,
    )?;
    git(repo.path(), &["add", "."])?;
    git(repo.path(), &[
        "commit",
        "-m",
        "develop publication",
    ])?;
    let first = git(repo.path(), &["rev-parse", "HEAD"])?;
    let remote = tempfile::tempdir()?;
    let bare = remote.path().join("remote.git");
    git(repo.path(), &[
        "clone",
        "--bare",
        ".",
        bare.to_str().unwrap(),
    ])?;
    let config = output.path().join("first.yaml");
    configuration(&config, &bare, &first, &definitions)?;
    let (dataset, plans) =
        showcases::revision_build(&config, cache.path()).await?;
    assert!(bodies(&dataset, "MAIN MUST NEVER APPEAR").await?.is_empty());
    assert!(!bodies(&dataset, "DEVELOP ONE").await?.is_empty());
    let mut assets = dataset.select("SELECT (COUNT(DISTINCT ?asset) AS ?count) WHERE { GRAPH ?g { ?asset a <https://ekgf.org/ontology/grapharch#BinaryAsset> } }".into()).await?;
    let row = assets.next().await?.unwrap();
    let oxrdf::Term::Literal(count) = &row["count"] else {
        panic!("asset count literal")
    };
    assert_eq!(count.value(), "3");
    let expected_svg = std::fs::read(repo.path().join("badge.svg"))?;
    let mut routes = dataset.select("SELECT DISTINCT ?route WHERE { GRAPH ?g { ?asset a <https://ekgf.org/ontology/grapharch#BinaryAsset> ; <https://ekgf.org/ontology/grapharch#format> 'image/svg+xml' ; <https://ekgf.org/ontology/grapharch#route> ?route } }".into()).await?;
    let row = routes.next().await?.unwrap();
    let oxrdf::Term::Literal(svg_route) = &row["route"] else {
        panic!("SVG route literal")
    };
    let svg_route = svg_route.value().to_owned();
    assert!(routes.next().await?.is_none());
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    std::fs::write(
        repo.path().join("README.md"),
        "# Development documentation\n\nDEVELOP TWO\n",
    )?;
    git(repo.path(), &["add", "README.md"])?;
    git(repo.path(), &["commit", "-m", "develop update"])?;
    let second = git(repo.path(), &["rev-parse", "HEAD"])?;
    git(repo.path(), &[
        "push",
        bare.to_str().unwrap(),
        "develop",
    ])?;
    let second_config = output.path().join("second.yaml");
    configuration(&second_config, &bare, &second, &definitions)?;
    let (updated, _) =
        showcases::revision_build(&second_config, cache.path()).await?;
    assert!(!bodies(&updated, "DEVELOP TWO").await?.is_empty());
    // The first exact commit stays pinned after its branch advances.
    let (pinned, pinned_plans) =
        showcases::revision_build(&config, cache.path()).await?;
    assert_eq!(pinned_plans.len(), 3);
    assert!(!bodies(&pinned, "DEVELOP ONE").await?.is_empty());
    assert!(
        bodies(&pinned, "DEVELOP TWO").await?.is_empty(),
        "pinned collection followed a moving branch"
    );
    std::fs::remove_file(repo.path().join("badge.svg"))?;
    git(repo.path(), &["add", "-u"])?;
    git(repo.path(), &[
        "commit",
        "-m",
        "missing required image",
    ])?;
    let third = git(repo.path(), &["rev-parse", "HEAD"])?;
    git(repo.path(), &[
        "push",
        bare.to_str().unwrap(),
        "develop",
    ])?;
    let third_config = output.path().join("third.yaml");
    configuration(&third_config, &bare, &third, &definitions)?;
    let failure = showcases::revision_build(&third_config, cache.path())
        .await
        .err()
        .ok_or_else(|| anyhow::anyhow!("missing Git image must fail"))?;
    assert!(
        format!("{failure:#}").contains("missing required Git blob badge.svg")
    );
    std::fs::remove_dir_all(repo.path())?;
    std::fs::remove_dir_all(remote.path())?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output.path())
            .await?;
    assert_eq!(publications.len(), 3);
    for target in ["website", "specification", "pdf"] {
        assert_eq!(
            std::fs::read(output.path().join(target).join(&svg_route))?,
            expected_svg,
            "SVG XML bytes must survive KG export/restore without rewriting"
        );
    }
    let spec = tokio::fs::read_to_string(
        output.path().join("specification/index.html"),
    )
    .await?;
    assert!(
        !spec.contains("RELATED PROFILE TITLE"),
        "header must use its designated ontology source"
    );
    assert!(spec.contains("The Data Product (DPROD) Ontology"));
    assert!(spec.contains("Remote develop legal terms, retained in full."));
    assert!(
        spec.contains("PINNED EXAMPLE SOURCE"),
        "machine-readable example source must survive source deletion"
    );
    let typst =
        tokio::fs::read_to_string(output.path().join("pdf/main.typ")).await?;
    assert!(typst.contains("PINNED EXAMPLE SOURCE"));
    assert!(spec.contains("retained-enumeration-member"));
    assert!(spec.contains("SOURCE OWNED FORMAL SCOPE"));
    assert!(typst.contains("SOURCE OWNED FORMAL SCOPE"));
    assert!(
        spec.find("SOURCE OWNED FORMAL SCOPE") <
            spec.find("PINNED EXAMPLE SOURCE")
    );
    let site =
        std::fs::read_to_string(output.path().join("website/index.html"))?;
    assert!(site.contains("href=\"assets/image-"));
    assert!(site.contains("src=\"assets/image-"));
    assert!(
        showcases::revision_build(&config, cache.path())
            .await
            .is_err(),
        "missing transport must not fall back to cached or copied sources"
    );
    Ok(())
}

#[tokio::test]
async fn remote_transport_rejects_oversized_settings_and_mutable_refs_before_network()
-> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let config = root.path().join("remote.yaml");
    let cache = root.path().join("cache");
    std::fs::write(&config, vec![b' '; 64 * 1024 + 1])?;
    assert!(
        format!(
            "{:#}",
            showcases::revision_build(&config, &cache)
                .await
                .err()
                .unwrap()
        )
        .contains("64 KiB")
    );
    let exact = "39155734ed3f79578e460e63eab6d31e9913206c";
    for (commit, definitions, rejected) in [
        ("main", exact, "showcase commit requires"),
        ("develop", exact, "showcase commit requires"),
        (
            exact,
            "develop",
            "showcase definitions commit requires",
        ),
        (&exact[..12], exact, "showcase commit requires"),
    ] {
        std::fs::write(
            &config,
            format!(
                "repository: https://example.invalid/never-fetch.git\ncommit: \
                 {commit}\ndefinitions:\n  commit: {definitions}\n  path: \
                 publication/definitions/develop.ttl\nproject: \
                 urn:dprod-showcase:project\ntargets: all\n"
            ),
        )?;
        let error = format!(
            "{:#}",
            showcases::revision_build(&config, &cache)
                .await
                .err()
                .unwrap()
        );
        assert!(error.contains(rejected), "{error}");
    }
    assert!(!cache.exists());
    Ok(())
}
