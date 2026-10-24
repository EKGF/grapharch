use {
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        dataset::Dataset,
        plan::TargetPlanner,
        term,
        writer::GraphWriter,
    },
    oxrdf::{Literal, NamedNode},
};

#[tokio::test]
async fn website_and_pdf_reuse_the_same_fragment_with_independent_positions()
-> anyhow::Result<()> {
    let project = "urn:example:ontology-project";
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new(
        project,
        "urn:collector:definitions",
        "tests/fixtures/publication/projects.ttl".into(),
    )?
    .collect(&writer)
    .await?;
    let assembler = Assembler::new(dataset.clone());
    let revision = assembler.assemble(project).await?;
    let planner = TargetPlanner::new(dataset.clone(), revision.clone());
    for (target, order) in [("spec", 1u32), ("pdf", 2u32)] {
        let plan = planner
            .resolve(
                project,
                &NamedNode::new(format!("urn:example:{target}"))?,
                &writer,
            )
            .await?;
        let snapshot = plan.snapshot.clone();
        let plan = plan.id;
        let selected = Dataset::logical_pattern(
            &snapshot,
            &format!("{} {} ?entry", plan, term("entry")),
            0,
        );
        let detail = Dataset::logical_pattern(
            &snapshot,
            &format!(
                "?entry {} <urn:example:intro-fragment> ; {} {}",
                term("fragment"),
                term("order"),
                Literal::from(order)
            ),
            1,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT ?entry WHERE {{ {selected} {detail} }}"
            ))
            .await?;
        assert!(rows.next().await?.is_some());
    }
    assert_eq!(revision, assembler.assemble(project).await?);
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn missing_references_and_overlapping_destinations_fail_explicitly()
-> anyhow::Result<()> {
    for suffix in [
        "\nex:intro-fragment ga:reference <urn:missing> .",
        "\nex:reader ga:destination \"build/spec/reader\" .",
    ] {
        let project = "urn:example:ontology-project";
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("project.ttl");
        let mut source =
            include_str!("fixtures/publication/projects.ttl").to_owned();
        if suffix.contains("destination") {
            source = source.replace(
                "ga:destination \"build/reader\"",
                "ga:destination \"build/spec/reader\"",
            );
        } else {
            source.push_str(suffix);
        }
        tokio::fs::write(&path, source).await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
        RdfFile::new(project, "urn:collector:definitions", path)?
            .collect(&writer)
            .await?;
        let revision =
            Assembler::new(dataset.clone()).assemble(project).await?;
        let result = TargetPlanner::new(dataset, revision)
            .resolve(
                project,
                &NamedNode::new("urn:example:spec")?,
                &writer,
            )
            .await;
        assert!(result.is_err());
        drop(writer);
        worker.await??;
    }
    Ok(())
}

#[tokio::test]
async fn a_reference_to_profile_content_excluded_by_scope_is_rejected()
-> anyhow::Result<()> {
    let project = "urn:example:ontology-project";
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("project.ttl");
    let source = format!(
        "{}\nex:intro-fragment ga:reference ex:internal . ex:spec-profile \
         ga:placement ex:hidden-placement . ex:hidden-placement a \
         ga:Placement ; ga:component ex:hidden-component ; ga:order 2 . \
         ex:hidden-component a ga:Component ; ga:fragment ex:hidden-fragment \
         . ex:hidden-fragment a ga:Fragment ; ga:subject ex:internal ; \
         ga:body \"Hidden\" ; ga:format \"text/markdown\" ; ga:contribution \
         ex:authored .",
        include_str!("fixtures/publication/projects.ttl")
    );
    tokio::fs::write(&path, source).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new(project, "urn:collector:definitions", path)?
        .collect(&writer)
        .await?;
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    assert!(
        TargetPlanner::new(dataset, revision)
            .resolve(
                project,
                &NamedNode::new("urn:example:spec")?,
                &writer
            )
            .await
            .is_err()
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn ambiguous_placement_and_fragment_order_are_rejected()
-> anyhow::Result<()> {
    for suffix in [
        "ex:spec-profile ga:placement ex:duplicate . ex:duplicate a \
         ga:Placement ; ga:component ex:introduction ; ga:order 1 .",
        "ex:introduction ga:fragment ex:second . ex:second a ga:Fragment ; \
         ga:subject ex:ontology-a ; ga:body \"Second fragment\" ; ga:format \
         \"text/markdown\" ; ga:contribution ex:authored .",
    ] {
        let project = "urn:example:ontology-project";
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("project.ttl");
        tokio::fs::write(
            &path,
            format!(
                "{}\n{suffix}",
                include_str!("fixtures/publication/projects.ttl")
            ),
        )
        .await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
        RdfFile::new(project, "urn:collector:definitions", path)?
            .collect(&writer)
            .await?;
        let revision =
            Assembler::new(dataset.clone()).assemble(project).await?;
        assert!(
            TargetPlanner::new(dataset, revision)
                .resolve(
                    project,
                    &NamedNode::new("urn:example:spec")?,
                    &writer
                )
                .await
                .is_err()
        );
        drop(writer);
        worker.await??;
    }
    Ok(())
}
