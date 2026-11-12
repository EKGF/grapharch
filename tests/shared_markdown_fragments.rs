use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, MarkdownFile},
        dataset::Dataset,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        view::RevisionView,
        website,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
};

const DEFINITIONS: &str = r#"
@prefix ga: <https://ekgf.org/ontology/grapharch#> .
@prefix dct: <http://purl.org/dc/terms/> .
<urn:project> a ga:Project; dct:title "Shared content"; ga:scope <urn:site-scope>; ga:target <urn:site>, <urn:pdf> .
<urn:site-scope> a ga:Scope .
<urn:site-profile> a ga:Profile; ga:theme ga:EkgfProjectTheme .
<urn:site> a ga:Target; dct:title "Shared website"; ga:scope <urn:site-scope>; ga:profile <urn:site-profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>; ga:destination "site" .
<urn:pdf-scope> a ga:Scope; ga:include <urn:shared:page>; ga:traverse ga:topic; ga:maxDepth 1 .
<urn:pdf-profile> a ga:Profile; ga:theme ga:OverviewTheme; ga:placement <urn:pdf-placement> .
<urn:pdf> a ga:Target; dct:title "Shared overview"; ga:scope <urn:pdf-scope>; ga:profile <urn:pdf-profile>; ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer>; ga:destination "pdf" .
<urn:pdf-placement> a ga:Placement; ga:component <urn:shared:component>; ga:order 1 .
<urn:shared:component> a ga:Component; dct:title "Shared explanation"; ga:fragment <urn:shared:section> .
"#;

fn source(sentence: &str) -> String {
    format!(
        "---\ntitle: Shared explanation\npermalink: /\ngrapharch:\n  subject: \
         urn:shared:page\n  fragment: urn:shared:whole\n  fragments:\n    \
         shared-explanation: urn:shared:section\n---\n# Shared \
         explanation\n\n{sentence}\n\n## Website details\n\nAdditional \
         website-only detail.\n"
    )
}

#[tokio::test]
async fn one_canonical_edit_updates_html_and_pdf_from_shared_restored_fragments()
-> Result<()> {
    for sentence in [
        "The original canonical explanation.",
        "The revised canonical explanation.",
    ] {
        let input = tempfile::tempdir()?;
        let settings = input.path().join("project.ttl");
        tokio::fs::write(&settings, DEFINITIONS).await?;
        tokio::fs::write(
            input.path().join("index.md"),
            source(sentence).replace(
                sentence,
                &format!(
                    "{sentence}\n\n![Shared illustration](assets/model.svg)"
                ),
            ),
        )
        .await?;
        tokio::fs::create_dir(input.path().join("assets")).await?;
        tokio::fs::write(input.path().join("assets/model.svg"), r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60"><rect width="120" height="60" fill="#4949b5"/></svg>"##).await?;
        let collected = website::collect(
            input.path(),
            &settings,
            "urn:project",
            "urn:site",
            2,
        )
        .await?;
        let dataset = collected.dataset;
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for("urn:project").await?,
        );
        let section = NamedNode::new("urn:shared:section")?;
        assert!(view.text(&section, "body").await?.contains(sentence));
        assert_eq!(
            view.node(&section, "derivedFrom").await?.as_str(),
            "urn:shared:whole"
        );
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
        let revision = Assembler::new(dataset.clone())
            .assemble("urn:project")
            .await?;
        let pdf = TargetPlanner::new(dataset.clone(), revision)
            .resolve(
                "urn:project",
                &NamedNode::new("urn:pdf")?,
                &writer,
            )
            .await?;
        drop(writer);
        worker.await??;
        let export = tempfile::tempdir()?;
        let archive = export.path().join("kg.nq");
        dataset.export(archive.clone()).await?;
        input.close()?;
        let restored = Dataset::memory()?;
        restored.import(archive).await?;
        let output = tempfile::tempdir()?;
        for (plan, file) in
            [(&collected.plan, "index.html"), (&pdf, "main.typ")]
        {
            let PublishOutcome::Published(publication) =
                Publisher::new(restored.clone())
                    .render(plan, output.path())
                    .await?
            else {
                panic!("publication required")
            };
            let body =
                tokio::fs::read_to_string(publication.directory.join(file))
                    .await?;
            assert!(
                body.contains(sentence),
                "{file} did not use the canonical fragment"
            );
            assert!(
                publication.manifest.files.iter().any(|path| {
                    path.starts_with("assets/") && path.ends_with(".svg")
                }),
                "{file} must retain its local image"
            );
            if file == "main.typ" {
                assert!(!body.contains("Additional website-only detail."));
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn invalid_fragment_selections_fail_explicitly() -> Result<()> {
    for (hints, message) in [
        (
            "    missing-heading: urn:shared:section",
            "missing Markdown section anchor",
        ),
        (
            "    shared-explanation: urn:shared:whole",
            "distinct from the whole fragment",
        ),
        (
            "    shared-explanation: urn:shared:section\n    website-details: \
             urn:shared:section",
            "duplicate Markdown fragment IRI",
        ),
    ] {
        let input = tempfile::tempdir()?;
        let path = input.path().join("source.md");
        let body = source("Canonical prose.").replace(
            "    shared-explanation: urn:shared:section",
            hints,
        );
        tokio::fs::write(&path, body).await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) = GraphWriter::spawn(dataset, 4, 1024 * 1024)?;
        let result = MarkdownFile::new("urn:project", "urn:collector", path)?
            .collect(&writer)
            .await;
        drop(writer);
        worker.await??;
        assert!(format!("{:#}", result.unwrap_err()).contains(message));
    }
    Ok(())
}

#[tokio::test]
async fn standalone_and_website_collectors_keep_the_same_fragment_identities()
-> Result<()> {
    let input = tempfile::tempdir()?;
    let path = input.path().join("index.md");
    tokio::fs::write(&path, source("Canonical source text.")).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    MarkdownFile::new(
        "urn:standalone",
        "urn:standalone:collector",
        path.clone(),
    )?
    .collect(&writer)
    .await?;
    MarkdownFile::in_site(
        "urn:website",
        "urn:website:collector",
        input.path().to_path_buf(),
        path,
    )?
    .collect(&writer)
    .await?;
    for project in ["urn:standalone", "urn:website"] {
        let view = RevisionView::new(
            dataset.clone(),
            dataset.snapshot_for(project).await?,
        );
        let whole = NamedNode::new("urn:shared:whole")?;
        let section = NamedNode::new("urn:shared:section")?;
        assert!(
            view.text(&whole, "body")
                .await?
                .contains("Canonical source text.")
        );
        assert!(
            view.text(&section, "body")
                .await?
                .contains("Canonical source text.")
        );
        assert_eq!(view.node(&section, "derivedFrom").await?, whole);
    }
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn local_markdown_images_reject_missing_and_escaping_paths() -> Result<()>
{
    for destination in ["assets/missing.svg", "../outside.svg"] {
        let parent = tempfile::tempdir()?;
        let root = parent.path().join("site");
        tokio::fs::create_dir(&root).await?;
        tokio::fs::write(
            parent.path().join("outside.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#,
        )
        .await?;
        let settings = root.join("project.ttl");
        tokio::fs::write(&settings, DEFINITIONS).await?;
        tokio::fs::write(
            root.join("index.md"),
            source(&format!("![Invalid image]({destination})")),
        )
        .await?;
        let result =
            website::collect(&root, &settings, "urn:project", "urn:site", 2)
                .await;
        let error = match result {
            Ok(_) => panic!("invalid local image must fail"),
            Err(error) => error,
        };
        let message = format!("{error:#}");
        assert!(
            message.contains(
                if destination.starts_with("..") {
                    "escapes documentation root"
                } else {
                    "local Markdown image"
                }
            ),
            "{message}"
        );
    }
    Ok(())
}
