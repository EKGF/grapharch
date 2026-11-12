use {anyhow::Result, grapharch::publication::hosting};

#[tokio::test]
async fn static_package_preserves_spec_assets_and_routes_versions_without_a_framework()
-> Result<()> {
    let input = tempfile::tempdir()?;
    tokio::fs::create_dir_all(input.path().join("site/guide")).await?;
    tokio::fs::create_dir_all(input.path().join("spec/assets")).await?;
    tokio::fs::create_dir_all(input.path().join("archive/1.0/assets")).await?;
    for (path, text) in [
        ("site/index.html", "Project home"),
        ("site/404.html", "Page not found"),
        ("site/guide/index.html", "Guide"),
        ("site/.nojekyll", ""),
        ("spec/index.html", "Current specification"),
        ("spec/assets/model.svg", "<svg>current</svg>"),
        (
            "spec/profile.ttl",
            "<urn:profile> a <urn:Profile> .",
        ),
        ("archive/1.0/index.html", "Frozen specification"),
        (
            "archive/1.0/assets/model.svg",
            "<svg>frozen</svg>",
        ),
    ] {
        tokio::fs::write(input.path().join(path), text).await?;
    }
    tokio::fs::write(input.path().join("inventory.json"), r#"{"observed_at":"2026-11-02T12:00:00Z","versions":[{"slug":"develop","branch":"develop","label":"Develop","url":"https://example.org/dprod/spec/develop/","advertised":true},{"slug":"main","branch":"main","label":"Frozen","url":"https://example.org/dprod/spec/main/","advertised":true},{"slug":"old-preview","branch":"old/preview","label":"Old preview","url":"https://immutable.vercel.app/dprod/spec/","advertised":false}]}"#).await?;
    let config = input.path().join("hosting.yml");
    tokio::fs::write(
        &config,
        "version: 2\nplatform: vercel\nmount: /dprod\nsite: \
         site\nstatic_trees:\n  - source: spec\n    destination: spec\n  - \
         source: archive\n    destination: spec/archive\ndirectory_aliases:\n  \
         - path: spec/develop\n    target: spec\n  - path: spec/main\n    \
         target: spec/archive/1.0\nfile_aliases:\n  - path: \
         spec/contracts/profile/\n    target: spec/profile.ttl\n    \
         content_type: text/turtle\ninventory: inventory.json\nversion_path: \
         spec\n",
    )
    .await?;
    let output = tempfile::tempdir()?;
    let package = hosting::package(&config, output.path(), None).await?;
    assert_eq!(
        tokio::fs::read(
            package.join("static/dprod/spec/main/assets/model.svg")
        )
        .await?,
        b"<svg>frozen</svg>"
    );
    assert_eq!(
        tokio::fs::read(
            package.join("static/dprod/spec/develop/assets/model.svg")
        )
        .await?,
        b"<svg>current</svg>"
    );
    assert!(
        tokio::fs::try_exists(package.join("static/dprod/.nojekyll")).await?
    );
    let routes: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(package.join("config.json")).await?,
    )?;
    assert_eq!(routes["version"], 3);
    assert_eq!(
        routes["routes"].as_array().unwrap().last().unwrap()["status"],
        404
    );
    assert_eq!(
        routes["routes"].as_array().unwrap().last().unwrap()["dest"],
        "/dprod/404.html"
    );
    let routes = routes["routes"].as_array().unwrap();
    assert!(routes.iter().any(|route| {
        route["src"] == "^/dprod/guide/$" &&
            route["dest"] == "/dprod/guide/index.html"
    }));
    assert!(routes.iter().any(|route| {
        route["src"] == "^/dprod/spec/contracts/profile/$" &&
            route["dest"] == "/dprod/spec/profile.ttl" &&
            route["headers"]["Content-Type"] == "text/turtle"
    }));
    assert!(routes.iter().any(|route| {
        route["headers"]["Location"] ==
            "https://immutable.vercel.app/dprod/spec/$1"
    }));
    assert!(!routes.iter().any(|route| {
        route["headers"]["Location"] ==
            "https://example.org/dprod/spec/develop/$1"
    }));
    assert!(!tokio::fs::try_exists(package.join("functions")).await?);
    assert!(
        hosting::package(&config, output.path(), None)
            .await
            .is_err(),
        "existing package must not be overwritten"
    );
    Ok(())
}

#[tokio::test]
async fn package_rejects_colliding_or_escaping_publication_paths() -> Result<()>
{
    for destination in ["../escape", "site/../../escape", "/absolute"] {
        let input = tempfile::tempdir()?;
        tokio::fs::create_dir(input.path().join("site")).await?;
        tokio::fs::write(input.path().join("site/index.html"), "Home").await?;
        let config = input.path().join("hosting.yml");
        tokio::fs::write(
            &config,
            format!(
                "version: 2\nplatform: vercel\nmount: /dprod\nsite: \
                 site\nstatic_trees:\n  - source: site\n    destination: \
                 {destination}\n"
            ),
        )
        .await?;
        let output = tempfile::tempdir()?;
        assert!(
            hosting::package(&config, output.path(), None)
                .await
                .is_err()
        );
        assert!(
            !tokio::fs::try_exists(output.path().join(".vercel/output"))
                .await?
        );
    }
    Ok(())
}

#[tokio::test]
async fn package_supports_generic_version_paths_and_root_artifact_aliases()
-> Result<()> {
    let input = tempfile::tempdir()?;
    tokio::fs::create_dir_all(input.path().join("site/manual")).await?;
    tokio::fs::write(input.path().join("site/index.html"), "Home").await?;
    tokio::fs::write(
        input.path().join("site/manual/index.html"),
        "Manual",
    )
    .await?;
    tokio::fs::write(input.path().join("inventory.json"), r#"{"observed_at":"2026-11-02T12:00:00Z","versions":[{"slug":"old","branch":"old","label":"Old","url":"https://example.org/manual/","advertised":false}]}"#).await?;
    let config = input.path().join("hosting.yml");
    tokio::fs::write(
        &config,
        "version: 2\nplatform: vercel\nmount: /project\nsite: \
         site\ndirectory_aliases:\n  - path: copy\n    target: \
         manual\ninventory: inventory.json\nversion_path: editions\n",
    )
    .await?;
    let output = tempfile::tempdir()?;
    let package = hosting::package(&config, output.path(), Some("/")).await?;
    assert_eq!(
        tokio::fs::read(package.join("static/copy/index.html")).await?,
        b"Manual"
    );
    let text = tokio::fs::read_to_string(package.join("config.json")).await?;
    assert!(text.contains("^/editions/old/(.*)$"));
    assert!(!text.contains("/spec/"));
    Ok(())
}

#[tokio::test]
async fn explicit_local_version_requires_materialized_publication() -> Result<()>
{
    let input = tempfile::tempdir()?;
    tokio::fs::create_dir(input.path().join("site")).await?;
    tokio::fs::write(input.path().join("site/index.html"), "Home").await?;
    tokio::fs::write(input.path().join("inventory.json"), r#"{"observed_at":"2026-11-02T12:00:00Z","versions":[{"slug":"main","branch":"main","label":"1.0 beta","url":"https://example.org/dprod/spec/main/","route":"/dprod/spec/main/","advertised":true}]}"#).await?;
    let config = input.path().join("hosting.yml");
    tokio::fs::write(
        &config,
        "version: 2\nplatform: vercel\nmount: /dprod\nsite: site\ninventory: \
         inventory.json\nversion_path: spec\n",
    )
    .await?;
    let output = tempfile::tempdir()?;
    let error = hosting::package(&config, output.path(), None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("no assembled index"));
    assert!(!output.path().join(".vercel/output").exists());
    tokio::fs::create_dir_all(input.path().join("site/spec/main")).await?;
    tokio::fs::write(
        input.path().join("site/spec/main/index.html"),
        "Beta",
    )
    .await?;
    let package = hosting::package(&config, output.path(), None).await?;
    let routes = tokio::fs::read_to_string(package.join("config.json")).await?;
    assert!(!routes.contains("example.org"));
    Ok(())
}

async fn pages_input(
    config: &str,
) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let input = tempfile::tempdir()?;
    tokio::fs::create_dir_all(input.path().join("site/guide")).await?;
    tokio::fs::create_dir_all(input.path().join("spec")).await?;
    for (path, text) in [
        ("site/index.html", "Project home"),
        ("site/404.html", "Page not found"),
        ("site/guide/index.html", "Guide"),
        ("spec/index.html", "Specification"),
        (
            "spec/profile.ttl",
            "<urn:profile> a <urn:Profile> .",
        ),
    ] {
        tokio::fs::write(input.path().join(path), text).await?;
    }
    let path = input.path().join("hosting.yml");
    tokio::fs::write(&path, config).await?;
    Ok((input, path))
}

const PAGES: &str = "version: 2\nplatform: github-pages\nmount: /dprod\nsite: \
                     site\nstatic_trees:\n  - source: spec\n    destination: \
                     spec/main\ndirectory_aliases:\n  - path: spec\n    \
                     target: spec/main\n";

#[tokio::test]
async fn github_pages_package_is_a_plain_static_tree_without_vercel_paths()
-> Result<()> {
    let (_input, config) = pages_input(PAGES).await?;
    let output = tempfile::tempdir()?;
    let package = hosting::package(&config, output.path(), None).await?;
    assert_eq!(
        package,
        output.path().canonicalize()?.join("static")
    );
    for (path, text) in [
        ("dprod/index.html", "Project home"),
        ("dprod/404.html", "Page not found"),
        ("dprod/guide/index.html", "Guide"),
        ("dprod/spec/main/index.html", "Specification"),
        ("dprod/spec/index.html", "Specification"),
    ] {
        assert_eq!(
            tokio::fs::read_to_string(package.join(path)).await?,
            text
        );
    }
    assert!(!package.join("config.json").exists());
    assert!(!output.path().join(".vercel").exists());
    let mut entries = tokio::fs::read_dir(output.path()).await?;
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        names.push(entry.file_name().into_string().unwrap());
    }
    assert_eq!(
        names,
        ["static"],
        "no staging or provider directories remain"
    );
    assert!(
        hosting::package(&config, output.path(), None)
            .await
            .is_err(),
        "existing package must not be overwritten"
    );
    Ok(())
}

#[tokio::test]
async fn github_pages_rejects_routing_it_cannot_serve() -> Result<()> {
    for (extra, expected) in [
        (
            "file_aliases:\n  - path: spec/contracts/profile/\n    target: \
             spec/main/profile.ttl\n    content_type: text/turtle\n",
            "spec/contracts/profile/",
        ),
        (
            "inventory: inventory.json\nversion_path: spec\n",
            "old-preview",
        ),
    ] {
        let (input, config) = pages_input(&format!("{PAGES}{extra}")).await?;
        tokio::fs::write(input.path().join("inventory.json"), r#"{"observed_at":"2026-11-02T12:00:00Z","versions":[{"slug":"main","branch":"main","label":"Main","url":"https://example.org/dprod/spec/main/","advertised":true},{"slug":"old-preview","branch":"old/preview","label":"Old","url":"https://preview.example.org/dprod/spec/","advertised":false}]}"#).await?;
        let output = tempfile::tempdir()?;
        let error = format!(
            "{:#}",
            hosting::package(&config, output.path(), None)
                .await
                .unwrap_err()
        );
        assert!(
            error.contains("github-pages") && error.contains(expected),
            "{error}"
        );
        assert!(!output.path().join("static").exists());
    }
    Ok(())
}

#[tokio::test]
async fn hosting_platform_is_explicit_and_never_defaults_to_vercel()
-> Result<()> {
    for config in [
        "version: 2\nmount: /dprod\nsite: site\n",
        "version: 2\nplatform: netlify\nmount: /dprod\nsite: site\n",
        "version: 1\nplatform: github-pages\nmount: /dprod\nsite: site\n",
    ] {
        let (_input, path) = pages_input(config).await?;
        let output = tempfile::tempdir()?;
        assert!(
            hosting::package(&path, output.path(), None).await.is_err(),
            "accepted {config}"
        );
        assert!(!output.path().join(".vercel").exists());
        assert!(!output.path().join("static").exists());
    }
    Ok(())
}
