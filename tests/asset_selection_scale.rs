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
    tokio::io::AsyncBufReadExt,
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

/// Replay an explicitly supplied real KG/query pair without network or source
/// I/O.
#[tokio::test]
#[ignore = "diagnostic: requires GRAPHARCH_ASSET_PROFILE_KG and \
            GRAPHARCH_ASSET_PROFILE_TRACE"]
async fn replay_exported_asset_selector_before_and_after_semijoin() -> Result<()>
{
    let archive = std::env::var("GRAPHARCH_ASSET_PROFILE_KG")?;
    let mut trace = tokio::io::BufReader::new(
        tokio::fs::File::open(std::env::var("GRAPHARCH_ASSET_PROFILE_TRACE")?)
            .await?,
    )
    .lines();
    let mut old = None;
    // Dataset SELECT text is capped at 64 KiB; keep only one matching query,
    // rather than buffering a complete trace or the source corpus.
    while let Some(line) = trace.next_line().await? {
        anyhow::ensure!(
            line.len() <= 68 * 1024,
            "diagnostic trace line exceeds query budget"
        );
        if let Some(query) =
            line.strip_prefix("TRACE executing SPARQL SELECT query=") &&
            query.starts_with("SELECT DISTINCT ?asset WHERE { { ") &&
            query.contains("#derivedFrom>")
        {
            old = Some(query.to_owned());
        }
    }
    let old = old.expect("trace must contain the scoped asset selector");
    let start = old.find("{ {").unwrap() + 2;
    let mut depth = 0;
    let mut end = None;
    for (offset, character) in old[start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + offset);
                    break;
                }
            },
            _ => {},
        }
    }
    let end = end.unwrap();
    let candidates = &old[start + 1..end];
    let types = old[end + 1..].strip_suffix(" } ORDER BY ?asset").unwrap();
    let candidates = regex::Regex::new(r"BIND\((<[^>]+>) AS \?asset\)")?
        .replace_all(candidates, "FILTER(?asset = $1)");
    let rewritten = format!(
        "SELECT DISTINCT ?asset WHERE {{ {types} FILTER EXISTS {{ \
         {candidates} }} }} ORDER BY ?asset"
    );
    let dataset = Dataset::memory()?;
    let started = Instant::now();
    dataset.import(archive.into()).await?;
    eprintln!("real KG imported in {:?}", started.elapsed());
    let only_rewrite =
        std::env::var_os("GRAPHARCH_ASSET_PROFILE_ONLY_NEW").is_some();
    let mut results = Vec::new();
    // bounded: exactly two queries, each at most the publication's 256 assets.
    for (label, query) in [
        ("old asset join", old.to_owned()),
        ("correlated asset semijoin", rewritten),
    ] {
        if only_rewrite && label == "old asset join" {
            continue;
        }
        let started = Instant::now();
        let mut rows = dataset.select(query).await?;
        let mut assets = BTreeSet::new();
        while let Some(row) = rows.next().await? {
            anyhow::ensure!(
                assets.len() <= 256,
                "unexpected diagnostic asset count"
            );
            assets.insert(row["asset"].to_string());
        }
        eprintln!(
            "{label}: {:?}, {} selected assets",
            started.elapsed(),
            assets.len()
        );
        results.push(assets);
    }
    if !only_rewrite {
        assert_eq!(results[0], results[1]);
    }
    Ok(())
}
