#[path = "support/definitions.rs"]
mod definitions;

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
    let fixture = definitions::set_property(
        include_str!("../examples/showcase/project.ttl"),
        "urn:showcase:pdf",
        "https://ekgf.org/ontology/grapharch#profile",
        definitions::iri("urn:showcase:reader-profile")?,
    )?;
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
        definitions::set_property(
            include_str!("../examples/specification/project.ttl"),
            "urn:specification:profile",
            "https://ekgf.org/ontology/grapharch#style",
            definitions::literal("reader"),
        )?,
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
    assert!(
        typst.contains("#let ga-ribbon = \"EKGF Editor's Draft\""),
        "the PDF ribbon must use the pinned authority short name and \
         publication status"
    );
    assert!(typst.contains("rotate(-90deg, reflow: true)"));
    assert!(typst.contains("#ga-cover("));
    assert!(
        typst.contains("#ga-section(1)["),
        "component boundaries must carry heading space outside their fragment \
         blocks"
    );
    let footer = typst
        .split("footer: context {")
        .nth(1)
        .expect("running footer")
        .split("  })")
        .next()
        .expect("footer body");
    assert!(
        !footer.contains("ga-publisher"),
        "authority belongs in the header, not twice on each page"
    );
    assert!(typst.contains("Editor's Draft"));
    assert!(typst.contains("Publication requirements"));
    let metadata = std::fs::read(publication.directory.join("document.pdf"))?;
    assert!(metadata.starts_with(b"%PDF-"));
    Ok(())
}

#[tokio::test]
async fn unsupported_pdf_style_is_rejected() -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    std::fs::write(
        &path,
        definitions::set_property(
            include_str!("../examples/showcase/project.ttl"),
            "urn:showcase:spec-profile",
            "https://ekgf.org/ontology/grapharch#style",
            definitions::literal("unsupported"),
        )?,
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
    let fixture = definitions::set_property(
        original,
        "urn:showcase:pdf",
        "https://ekgf.org/ontology/grapharch#publisher",
        definitions::literal("invalid publisher"),
    )?;
    std::fs::write(&path, fixture)?;
    let (dataset, plans) = showcases::authored(&path).await?;
    let out = tempfile::tempdir()?;
    let result = Publisher::new(dataset).render(&plans[2], out.path()).await;
    let error = result.err().expect("publisher literals must fail");
    assert!(format!("{error:#}").contains("publisher must be an IRI"));
    Ok(())
}

#[tokio::test]
async fn long_authority_and_status_fit_the_pdf_ribbon() -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let authority =
        "International Council for Open Standards and Interoperability";
    let status =
        "Public review documentation preview for implementing organizations";
    let fixture = definitions::set_property(
        include_str!("../examples/specification/project.ttl"),
        "urn:uuid:6c1408aa-039c-42f9-8dee-9c3f0009d61a",
        "https://ekgf.org/ontology/grapharch#shortName",
        definitions::literal(authority),
    )?;
    let fixture = definitions::set_property(
        &fixture,
        "urn:specification:target",
        "https://ekgf.org/ontology/grapharch#publicationStatus",
        definitions::literal(status),
    )?;
    std::fs::write(&path, fixture)?;
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
    assert!(typst.contains(&format!(
        "#let ga-ribbon = \"{authority} {status}\""
    )));
    assert!(
        typst.contains("(ribbon-height - 12mm) / ribbon-measure"),
        "ribbon text must fit its available measure without truncating \
         metadata"
    );
    assert!(
        std::fs::read(publication.directory.join("document.pdf"))?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[tokio::test]
async fn independent_standards_pdf_preserves_publication_metadata() -> Result<()>
{
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = definitions::set_property(
        include_str!("../examples/specification/project.ttl"),
        "urn:specification:target",
        "https://ekgf.org/ontology/grapharch#renderer",
        definitions::literal("typst"),
    )?;
    let fixture =
        format!("{fixture}\nex:profile ga:style \"specification\" .\n");
    std::fs::write(&path, fixture)?;
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
    assert!(
        typst.contains("Alex Example"),
        "standalone standards PDF must retain its editor"
    );
    assert!(
        typst.contains("https://example.org/spec/2026-10-24/"),
        "standalone standards PDF must retain its version URL"
    );
    Ok(())
}
