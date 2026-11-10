use {
    anyhow::Result,
    grapharch::publication::{Renderer, Target},
    oxigraph::{
        io::{RdfFormat, RdfParser},
        store::Store,
    },
    oxrdf::NamedNode,
};

fn target(renderer: &str) -> Result<Target> {
    let store = Store::new()?;
    store.load_from_reader(
        RdfParser::from_format(RdfFormat::Turtle),
        format!(
            "@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:t> \
             ga:scope <urn:s>; ga:profile <urn:p>; ga:destination \"site\"; \
             ga:renderer {renderer}. <urn:custom:WebsiteRenderer> a \
             ga:Renderer."
        )
        .as_bytes(),
    )?;
    Target::read(&store, NamedNode::new("urn:t")?)
}

#[test]
fn renderer_definitions_have_exact_shared_iri_identities() -> Result<()> {
    for (name, expected) in [
        ("HtmlRenderer", Renderer::Html),
        ("WebsiteRenderer", Renderer::Website),
        ("SpecificationRenderer", Renderer::Specification),
        (
            "SpecificationPagesRenderer",
            Renderer::SpecificationPages,
        ),
        ("BookshelfRenderer", Renderer::Bookshelf),
        ("TypstRenderer", Renderer::Typst),
    ] {
        assert_eq!(target(&format!("ga:{name}"))?.renderer, expected);
    }
    Ok(())
}

#[test]
fn literals_and_unknown_renderer_iris_are_rejected() {
    for renderer in [
        "\"website\"",
        "<urn:custom:WebsiteRenderer>",
        "ga:UnknownRenderer",
        "[]",
    ] {
        assert!(target(renderer).is_err(), "accepted {renderer}");
    }
}

#[test]
fn fragment_renderer_cannot_be_a_publication_target() {
    let error = target("ga:OwlClassDiagramRenderer")
        .unwrap_err()
        .to_string();
    assert!(error.contains("fragment renderer"), "{error}");
    assert!(error.contains("publication target"), "{error}");
}

#[tokio::test]
async fn shared_definition_and_distinct_target_settings_survive_source_removal()
-> Result<()> {
    use grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        dataset::Dataset,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        term,
        view::RevisionView,
        writer::GraphWriter,
    };
    let source = tempfile::tempdir()?;
    let file = source.path().join("project.ttl");
    let fixture = include_str!("fixtures/publication/projects.ttl")
        .replace("#SpecificationRenderer>", "#HtmlRenderer>")
        .replace("#TypstRenderer>", "#HtmlRenderer>");
    tokio::fs::write(&file, fixture).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new(
        "urn:example:ontology-project",
        "urn:definitions",
        file,
    )?
    .collect(&writer)
    .await?;
    let assembler = Assembler::new(dataset.clone());
    let revision = assembler.assemble("urn:example:ontology-project").await?;
    assert_eq!(
        revision,
        assembler.assemble("urn:example:ontology-project").await?,
        "catalogue retention must be idempotent"
    );
    let planner = TargetPlanner::new(dataset.clone(), revision.clone());
    let mut plans = Vec::new(); // bounded: two configured target uses of one definition.
    for name in ["spec", "pdf"] {
        plans.push(
            planner
                .resolve(
                    "urn:example:ontology-project",
                    &NamedNode::new(format!("urn:example:{name}"))?,
                    &writer,
                )
                .await?,
        );
    }
    drop(writer);
    worker.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("graph.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let view = RevisionView::new(restored.clone(), revision);
    assert!(view.has_type(&Renderer::Html.iri(), "Renderer").await?);
    assert!(
        view.has_type(&Renderer::Html.iri(), "PublicationRenderer")
            .await?
    );
    assert!(
        view.has_type(
            &term("OwlClassDiagramRenderer"),
            "FragmentRenderer"
        )
        .await?
    );
    assert!(
        target("ga:OwlClassDiagramRenderer").is_err(),
        "fragment renderers cannot be publication targets"
    );
    let mut labels = view
        .objects_by(
            &Renderer::Html.iri(),
            &NamedNode::new("http://www.w3.org/2000/01/rdf-schema#label")?,
        )
        .await?;
    assert!(labels.next().await?.is_some());
    for plan in &plans {
        let pinned = RevisionView::new(restored.clone(), plan.snapshot.clone());
        assert_eq!(
            pinned.node(&plan.id, "renderer").await?,
            term("HtmlRenderer")
        );
    }
    let publisher = Publisher::new(restored);
    let mut destinations = Vec::new(); // bounded: two fixture publications.
    for plan in plans {
        let PublishOutcome::Published(artifact) =
            publisher.render(&plan, output.path()).await?
        else {
            panic!("publication required")
        };
        assert!(
            tokio::fs::try_exists(artifact.directory.join("index.html"))
                .await?
        );
        destinations.push(artifact.directory);
    }
    assert_ne!(destinations[0], destinations[1]);
    Ok(())
}
