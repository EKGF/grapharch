use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile, collect_all},
        dataset::Dataset,
        plan::TargetPlanner,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
    std::{collections::BTreeSet, sync::Arc, time::Instant},
};

#[tokio::test]
async fn asset_selection_ignores_unrelated_rdf_contribution_products()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(
        &definitions,
        format!(
            r#"{}
ex:scope ga:include ex:explicit-asset .
ex:subject ga:asset ex:direct-asset .
ex:intro-body ga:asset ex:attached-asset .
ex:model-body ga:derivedFrom ex:source-owner .
ex:source-owner ga:asset ex:inherited-asset .

ex:target ga:productLogo ex:logo-asset .
ex:unselected-fragment ga:subject ex:unselected ; ga:asset ex:hidden-asset .
ex:explicit-asset a ga:BinaryAsset ; ga:format "image/png" .
ex:direct-asset a ga:BinaryAsset ; ga:format "image/png" .
ex:attached-asset a ga:BinaryAsset ; ga:format "image/png" .
ex:inherited-asset a ga:BinaryAsset ; ga:format "image/png" .
ex:logo-asset a ga:BinaryAsset ; ga:format "image/png" .
ex:hidden-asset a ga:BinaryAsset ; ga:format "image/png" .
"#,
            include_str!("../examples/specification/project.ttl")
        ),
    )
    .await?;
    let mut configured = tokio::fs::read_to_string(&definitions).await?;
    for subject in 0..16 {
        configured.push_str(&format!(
            "ex:scope ga:include <urn:dependency:subject:{subject}> .\n"
        ));
    }
    tokio::fs::write(&definitions, configured).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let mut work: Vec<Arc<dyn Collector>> = vec![Arc::new(RdfFile::new(
        "urn:specification:project",
        "urn:asset-scale:definitions",
        definitions,
    )?)];
    // bounded: 32 unrelated source contributions, 16 native descriptions each.
    // The graph-membership join shape, rather than corpus bytes, caused the
    // bug.
    for contribution in 0..32 {
        let path = source.path().join(format!("dependency-{contribution}.ttl"));
        let mut data = String::new();
        for resource in 0..16 {
            data.push_str(&format!(
                "<urn:dependency:{contribution}:{resource}> \
                 <urn:dependency:predicate> \
                 <urn:dependency:object:{resource}> .\n"
            ));
        }
        tokio::fs::write(&path, data).await?;
        work.push(Arc::new(RdfFile::new(
            "urn:specification:project",
            &format!("urn:asset-scale:collector:{contribution}"),
            path,
        )?));
    }
    collect_all(work, &writer, 8).await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:specification:project")
        .await?;
    let started = Instant::now();
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(
            "urn:specification:project",
            &NamedNode::new("urn:specification:target")?,
            &writer,
        )
        .await?;
    let elapsed = started.elapsed();
    eprintln!("asset-selection scale fixture: {elapsed:?}");
    let pattern = Dataset::logical_pattern(
        &plan.snapshot,
        &format!(
            "{} <https://ekgf.org/ontology/grapharch#asset> ?asset",
            plan.id
        ),
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?asset WHERE {{ {pattern} }}"
        ))
        .await?;
    // bounded: six fixture assets, five of which should be selected.
    let mut assets = BTreeSet::new();
    while let Some(row) = rows.next().await? {
        assets.insert(row["asset"].to_string());
    }
    let expected = ["explicit", "direct", "attached", "inherited", "logo"]
        .into_iter()
        .map(|kind| format!("<urn:specification:{kind}-asset>"))
        .collect();
    assert_eq!(
        assets, expected,
        "all existing asset attachment paths must remain selected, with \
         unselected subjects excluded"
    );
    drop(writer);
    worker.await??;
    Ok(())
}

#[tokio::test]
async fn empty_asset_selection_does_not_expand_native_dependency_graphs()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(
        &definitions,
        include_str!("../examples/specification/project.ttl"),
    )
    .await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let mut work: Vec<Arc<dyn Collector>> = vec![Arc::new(RdfFile::new(
        "urn:specification:project",
        "urn:empty-assets:definitions",
        definitions,
    )?)];
    // bounded: 355 tiny RDF contributions match a real documentation build's
    // source-head count. BinaryAsset is present as a schema resource, but no
    // selected subject owns an asset.
    for contribution in 0..355 {
        let path = source.path().join(format!("dependency-{contribution}.ttl"));
        tokio::fs::write(&path, format!("<urn:property:{contribution}> <http://www.w3.org/2000/01/rdf-schema#range> <https://ekgf.org/ontology/grapharch#BinaryAsset> .\n<urn:unselected:{contribution}> a <https://ekgf.org/ontology/grapharch#BinaryAsset> ; <https://ekgf.org/ontology/grapharch#format> \"image/png\" .\n")).await?;
        work.push(Arc::new(RdfFile::new(
            "urn:specification:project",
            &format!("urn:empty-assets:collector:{contribution}"),
            path,
        )?));
    }
    collect_all(work, &writer, 8).await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:specification:project")
        .await?;
    let started = Instant::now();
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(
            "urn:specification:project",
            &NamedNode::new("urn:specification:target")?,
            &writer,
        )
        .await?;
    let elapsed = started.elapsed();
    eprintln!("empty asset selection: {elapsed:?}");
    // The prior selector exceeded 129 seconds on this fixed-size fixture.
    // The guarded selector takes ~44 ms; this broad budget catches the join
    // explosion while leaving substantial headroom for slower CI machines.
    assert!(
        elapsed < std::time::Duration::from_secs(15),
        "empty asset selection expanded unrelated source heads: {elapsed:?}"
    );
    let pattern = Dataset::logical_pattern(
        &plan.snapshot,
        &format!(
            "{} <https://ekgf.org/ontology/grapharch#asset> ?asset",
            plan.id
        ),
        0,
    );
    let mut rows = dataset
        .select(format!("SELECT ?asset WHERE {{ {pattern} }}"))
        .await?;
    assert!(rows.next().await?.is_none());
    drop(writer);
    worker.await??;
    Ok(())
}
