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
    tokio::fs::write(&config, "version: 1\nmount: /dprod\nsite: site\nstatic_trees:\n  - source: spec\n    destination: spec\n  - source: archive\n    destination: spec/archive\ndirectory_aliases:\n  - path: spec/develop\n    target: spec\n  - path: spec/main\n    target: spec/archive/1.0\nfile_aliases:\n  - path: spec/contracts/profile/\n    target: spec/profile.ttl\n    content_type: text/turtle\ninventory: inventory.json\nversion_path: spec\n").await?;
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
                "version: 1\nmount: /dprod\nsite: site\nstatic_trees:\n  - \
                 source: site\n    destination: {destination}\n"
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
        "version: 1\nmount: /project\nsite: site\ndirectory_aliases:\n  - \
         path: copy\n    target: manual\ninventory: \
         inventory.json\nversion_path: editions\n",
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
