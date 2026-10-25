use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        showcases,
    },
};

#[tokio::test]
async fn reader_pdf_style_is_selected_from_the_pinned_profile() -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = include_str!("../examples/showcase/project.ttl").replace(
        "ga:profile ex:spec-profile ; ga:renderer \"typst\"",
        "ga:profile ex:reader-profile ; ga:renderer \"typst\"",
    );
    let fixture = fixture.replace("Literal brackets such as", r"## Coverage\n\n| Term | Count |\n| --- | ---: |\n| Document | 28 |\n\n> [!WARNING]\n> Review annotations must stay together.\n\nLiteral brackets such as");
    let long_iri = format!(
        "https://example.org/{}",
        "longidentifier".repeat(30)
    );
    let fixture = fixture.replace(
        "Literal brackets such as",
        &format!(r"```text\n{long_iri}\n```\n\nLiteral brackets such as"),
    );
    std::fs::write(&path, fixture)?;
    let (dataset, plans) = showcases::authored(&path).await?;
    let archive = source.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    std::fs::remove_file(path)?;
    let plan = &plans[2];
    let out = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) =
        Publisher::new(restored).render(plan, out.path()).await?
    else {
        panic!("publication required")
    };
    let typst =
        std::fs::read_to_string(publication.directory.join("main.typ"))?;
    assert!(
        typst.contains("#let ga-style = \"reader\""),
        "reader typography must be selected from its pinned publication \
         profile"
    );
    assert!(typst.contains("#let ga-accent = rgb(\"#116659\")"));
    assert!(typst.contains("Liberation Sans"));
    assert!(typst.contains("#let ga-code-text(value) = layout(size =>"));
    assert!(typst.contains("let capacity = calc.max(1"));
    assert!(
        typst.contains(&long_iri),
        "code source must stay complete"
    );
    assert!(typst.contains("header: context"));
    assert!(typst.contains("footer: context"));
    assert!(typst.contains("Documentation vocabulary"));
    assert!(
        typst.contains("#ga-table(columns: (1fr, auto), align: (left, right),"),
        "numeric Markdown column alignment must survive into the PDF"
    );
    assert!(
        typst.contains("#ga-callout(\"Warning\", kind: \"warning\")["),
        "callout titles must use the themed, keep-with-body component"
    );
    assert!(
        std::fs::read(publication.directory.join("document.pdf"))?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn paired_specification_pdf_uses_the_authority_theme() -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    std::fs::write(
        &path,
        include_str!("../examples/specification/project.ttl").replace(
            "ex:profile a ga:Profile ;",
            "ex:profile a ga:Profile ; ga:style \"reader\" ;",
        ),
    )?;
    let (dataset, plans) = showcases::specification(&path).await?;
    std::fs::remove_file(path)?;
    let out = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&plans[0], out.path())
        .await?
    else {
        panic!("publication required")
    };
    let typst =
        std::fs::read_to_string(publication.directory.join("main.typ"))?;
    assert!(typst.contains("#let ga-style = \"specification\""));
    assert!(typst.contains("#let ga-accent = rgb(\"#2b5f9f\")"));
    assert!(typst.contains("Editor's Draft"));
    assert!(typst.contains("Publication requirements"));
    Ok(())
}

#[tokio::test]
async fn unsupported_pdf_style_is_rejected() -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    std::fs::write(
        &path,
        include_str!("../examples/showcase/project.ttl").replace(
            "ga:style \"specification\"",
            "ga:style \"unsupported\"",
        ),
    )?;
    let (dataset, plans) = showcases::authored(&path).await?;
    let plan = &plans[2];
    let out = tempfile::tempdir()?;
    let error = Publisher::new(dataset)
        .render(plan, out.path())
        .await
        .err()
        .expect("unknown PDF styles must fail");
    assert!(
        format!("{error:#}").contains("unsupported PDF presentation style")
    );
    assert!(!out.path().join("review/document.pdf").exists());
    Ok(())
}

#[tokio::test]
async fn malformed_pdf_publisher_is_rejected() -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let original = include_str!("../examples/showcase/project.ttl");
    let fixture = original.replace(
        "ga:renderer \"typst\" ;",
        "ga:renderer \"typst\" ; ga:publisher \"invalid publisher\" ;",
    );
    assert_ne!(fixture, original);
    std::fs::write(&path, fixture)?;
    let (dataset, plans) = showcases::authored(&path).await?;
    let out = tempfile::tempdir()?;
    let result = Publisher::new(dataset).render(&plans[2], out.path()).await;
    let error = result.err().expect("publisher literals must fail");
    assert!(format!("{error:#}").contains("publisher must be an IRI"));
    Ok(())
}
