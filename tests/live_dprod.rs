use grapharch::publication::{dataset::Dataset, render::Publisher, showcases};
#[tokio::test]
#[ignore = "explicit remote develop acceptance; performs read-only network \
            collection"]
async fn remote_develop_restores_and_publishes_three_targets()
-> anyhow::Result<()> {
    let cache = std::path::Path::new("output/cache/git");
    let (dataset, plans) = showcases::dprod_remote(
        std::path::Path::new("examples/dprod/live.yaml"),
        cache,
    )
    .await?;
    let output = std::path::Path::new("output/live-dprod-acceptance");
    tokio::fs::create_dir_all(output).await?;
    let archive = output.join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output).await?;
    assert_eq!(publications.len(), 3);
    let html =
        tokio::fs::read_to_string(output.join("specification/index.html"))
            .await?;
    assert!(html.contains("Data Product"));
    assert!(
        tokio::fs::read(output.join("pdf/document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

fn git(root: &std::path::Path, args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "fixture git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
#[tokio::test]
async fn remote_develop_uses_source_owned_hints_and_cache_without_fixture_fallback()
-> anyhow::Result<()> {
    use std::path::Path;
    let repo = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    git(repo.path(), &["init", "-b", "main"])?;
    git(repo.path(), &["config", "user.name", "Fixture"])?;
    git(repo.path(), &[
        "config",
        "user.email",
        "fixture@example.org",
    ])?;
    std::fs::write(
        repo.path().join("README.md"),
        "# MAIN MUST NEVER APPEAR\n",
    )?;
    std::fs::write(
        repo.path().join("ontology.ttl"),
        "<urn:main> <urn:p> <urn:o> .",
    )?;
    git(repo.path(), &["add", "."])?;
    git(repo.path(), &["commit", "-m", "main trap"])?;
    git(repo.path(), &["checkout", "-b", "develop"])?;
    std::fs::create_dir_all(repo.path().join("publication"))?;
    std::fs::create_dir_all(repo.path().join("ontology/dprod"))?;
    for (source, target) in [
        (
            "sources/ontology.ttl",
            "ontology/dprod/dprod-ontology.ttl",
        ),
        (
            "sources/shapes.ttl",
            "ontology/dprod/dprod-shapes.ttl",
        ),
        ("abstract.md", "publication/abstract.md"),
        ("status.md", "publication/status.md"),
    ] {
        std::fs::copy(
            Path::new("examples/dprod").join(source),
            repo.path().join(target),
        )?;
    }
    std::fs::write(
        repo.path().join("README.md"),
        "# Development documentation\n\nDEVELOP ONE\n\n![Fixture \
         badge](badge.svg)\n\n[Download \
         badge](badge.svg)\n\n![PNG](tiny%20photo+%25.png)\n\n![JPEG](tiny.\
         jpg)\n",
    )?;
    std::fs::write(
        repo.path().join("badge.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#fff"/></svg>"##,
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
    let mut definitions =
        std::fs::read_to_string("examples/dprod/project.ttl")?
            .replace(
                " ; dct:title \"Data Product Ontology (DPROD)\"",
                "",
            )
            .replace(
                " ; dct:title \"DPROD ontology and SHACL reference\"",
                "",
            )
            .replace(
                "    ga:license \
                 <https://creativecommons.org/licenses/by/4.0/> ;\n",
                "",
            );
    definitions.push_str(
        "\nex:project ga:collector ex:c1,ex:c2,ex:c3,ex:c4,ex:c5,ex:c6 .\n",
    );
    for (id, path, subject) in [
        (
            "c1",
            "ontology/dprod/dprod-ontology.ttl",
            "https://www.omg.org/spec/DPROD/dprod/",
        ),
        (
            "c2",
            "ontology/dprod/dprod-shapes.ttl",
            "https://www.omg.org/spec/DPROD/shapes/",
        ),
    ] {
        definitions.push_str(&format!(
            "ex:{id} a ga:Collector ; ga:kind \"rdf\" ; ga:sourcePath \
             \"{path}\" ; ga:source \"{path}\" ; ga:subject <{subject}> .\n"
        ));
    }
    for (id, path, key, title, route) in [
        (
            "c3",
            "publication/abstract.md",
            "abstract",
            "Abstract",
            None,
        ),
        (
            "c4",
            "publication/status.md",
            "status",
            "Status of This Document",
            None,
        ),
        (
            "c5",
            "README.md",
            "overview",
            "Development documentation",
            Some(""),
        ),
        (
            "c6",
            "example.md",
            "example",
            "Dataset example",
            Some("guide/"),
        ),
    ] {
        definitions.push_str(&format!(
            "ex:{id} a ga:Collector ; ga:kind \"{}\" ; ga:sourcePath \
             \"{path}\" ; ga:source \"{path}\" ; ga:fragment ex:{key} ; \
             dct:identifier \"{key}\"",
            if route.is_some() {
                "markdown-page"
            } else {
                "markdown"
            }
        ));
        if let Some(route) = route {
            definitions.push_str(&format!(
                " ; dct:title \"{title}\" ; ga:route \"{route}\""
            ));
        }
        definitions.push_str(" .\n");
    }
    std::fs::write(
        repo.path().join("publication/grapharch.ttl"),
        definitions,
    )?;
    git(repo.path(), &["add", "."])?;
    git(repo.path(), &[
        "commit",
        "-m",
        "develop publication",
    ])?;
    let remote = tempfile::tempdir()?;
    let clone = std::process::Command::new("git")
        .args([
            "clone",
            "--bare",
            repo.path().to_str().unwrap(),
            remote.path().join("remote.git").to_str().unwrap(),
        ])
        .output()?;
    anyhow::ensure!(clone.status.success(), "bare clone failed");
    let config = output.path().join("remote.yaml");
    std::fs::write(
        &config,
        format!(
            "repository: {}\nref: develop\ndefinitions: \
             publication/grapharch.ttl\n",
            remote.path().join("remote.git").display()
        ),
    )?;
    let (dataset, plans) =
        showcases::dprod_remote(&config, cache.path()).await?;
    let mut rows=dataset.select("SELECT ?body WHERE { GRAPH ?g { ?f <https://ekgf.org/ontology/grapharch#body> ?body } }".into()).await?;
    let mut observed = false;
    while let Some(row) = rows.next().await? {
        if let oxrdf::Term::Literal(body) = &row["body"] {
            assert!(!body.value().contains("MAIN MUST NEVER APPEAR"));
            observed |= body.value().contains("DEVELOP ONE");
        }
    }
    assert!(observed);
    let mut assets=dataset.select("SELECT (COUNT(DISTINCT ?asset) AS ?count) WHERE { GRAPH ?g { ?asset a <https://ekgf.org/ontology/grapharch#BinaryAsset> } }".into()).await?;
    let row = assets.next().await?.unwrap();
    let oxrdf::Term::Literal(count) = &row["count"] else {
        panic!("asset count literal")
    };
    assert_eq!(count.value(), "3");
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    std::fs::write(
        repo.path().join("README.md"),
        "# Development documentation\n\nDEVELOP TWO\n",
    )?;
    git(repo.path(), &["add", "README.md"])?;
    git(repo.path(), &["commit", "-m", "develop update"])?;
    git(repo.path(), &[
        "push",
        remote.path().join("remote.git").to_str().unwrap(),
        "develop",
    ])?;
    let (updated, _) = showcases::dprod_remote(&config, cache.path()).await?;
    let mut rows=updated.select("SELECT ?body WHERE { GRAPH ?g { ?f <https://ekgf.org/ontology/grapharch#body> ?body } FILTER(CONTAINS(STR(?body),'DEVELOP TWO')) }".into()).await?;
    assert!(rows.next().await?.is_some());
    std::fs::remove_file(repo.path().join("badge.svg"))?;
    git(repo.path(), &["add", "-u"])?;
    git(repo.path(), &[
        "commit",
        "-m",
        "missing required image",
    ])?;
    git(repo.path(), &[
        "push",
        remote.path().join("remote.git").to_str().unwrap(),
        "develop",
    ])?;
    let failure = showcases::dprod_remote(&config, cache.path())
        .await
        .err()
        .ok_or_else(|| anyhow::anyhow!("missing Git image must fail"))?;
    assert!(
        format!("{failure:#}").contains("missing required Git blob badge.svg")
    );
    std::fs::remove_dir_all(repo.path())?;
    std::fs::remove_dir_all(remote.path())?;
    std::fs::remove_file(&config)?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output.path())
            .await?;
    assert_eq!(publications.len(), 3);
    let site =
        std::fs::read_to_string(output.path().join("website/index.html"))?;
    assert!(site.contains("href=\"assets/image-"));
    assert!(site.contains("src=\"assets/image-"));
    assert!(
        showcases::dprod_remote(&config, cache.path())
            .await
            .is_err(),
        "missing transport must not fall back to copied fixture"
    );
    Ok(())
}

#[tokio::test]
async fn remote_transport_rejects_oversized_settings_and_main_before_network()
-> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let config = root.path().join("remote.yaml");
    let cache = root.path().join("cache");
    std::fs::write(&config, vec![b' '; 64 * 1024 + 1])?;
    assert!(
        format!(
            "{:#}",
            showcases::dprod_remote(&config, &cache)
                .await
                .err()
                .unwrap()
        )
        .contains("64 KiB")
    );
    std::fs::write(
        &config,
        "repository: https://example.invalid/never-fetch.git\nref: \
         main\ndefinitions: publication/grapharch.ttl\n",
    )?;
    assert!(
        format!(
            "{:#}",
            showcases::dprod_remote(&config, &cache)
                .await
                .err()
                .unwrap()
        )
        .contains("must use develop")
    );
    assert!(!cache.exists());
    Ok(())
}
