//! Explicit real-KG selector profiling; no network or original-source reads.
use {
    anyhow::{Context, Result, ensure},
    grapharch::publication::dataset::Dataset,
    regex::Regex,
    std::{collections::BTreeSet, time::Instant},
    tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader},
};

async fn recorded_selector() -> Result<String> {
    let path = std::env::var("GRAPHARCH_ASSET_PROFILE_TRACE")?;
    let mut lines = BufReader::new(tokio::fs::File::open(path).await?).lines();
    let mut selected = None;
    // bounded: Dataset SELECT text is capped at 64 KiB; retain one query only.
    while let Some(line) = lines.next_line().await? {
        ensure!(
            line.len() <= 68 * 1024,
            "diagnostic trace line exceeds budget"
        );
        if let Some(query) =
            line.strip_prefix("TRACE executing SPARQL SELECT query=") &&
            query.starts_with("SELECT DISTINCT ?asset WHERE ") &&
            query.contains("#derivedFrom>")
        {
            selected = Some(query.to_owned());
        }
    }
    selected.context("trace contains no scoped asset selector")
}

fn context(query: &str, pattern: &str) -> Result<String> {
    Ok(Regex::new(pattern)?
        .find(query)
        .context("missing selector context")?
        .as_str()
        .to_owned())
}

async fn assets(dataset: &Dataset, query: String) -> Result<BTreeSet<String>> {
    let mut rows = dataset.select(query).await?;
    // bounded: a publication can select at most 256 assets.
    let mut selected = BTreeSet::new();
    while let Some(row) = rows.next().await? {
        selected.insert(row["asset"].to_string());
        ensure!(
            selected.len() <= 256,
            "diagnostic asset count exceeds budget"
        );
    }
    Ok(selected)
}

/// Supply an exported KG, its original trace, a guarded selector from the
/// current implementation's trace, and the expected asset count. The guarded
/// selector's three snapshot/scope identities are rebound to the old capture.
#[tokio::test]
#[ignore = "requires GRAPHARCH_ASSET_PROFILE_KG, _TRACE, _NEW_QUERY and \
            _EXPECTED_ASSETS"]
async fn guarded_selector_matches_the_captured_plan_without_replaying_the_slow_join()
-> Result<()> {
    let archive = std::env::var("GRAPHARCH_ASSET_PROFILE_KG")?;
    let expected: usize =
        std::env::var("GRAPHARCH_ASSET_PROFILE_EXPECTED_ASSETS")?.parse()?;
    let old = recorded_selector().await?;
    let file = tokio::fs::File::open(std::env::var(
        "GRAPHARCH_ASSET_PROFILE_NEW_QUERY",
    )?)
    .await?;
    let mut guarded = String::new();
    file.take(64 * 1024 + 1)
        .read_to_string(&mut guarded)
        .await?;
    ensure!(
        guarded.len() <= 64 * 1024 && guarded.contains("?_asset_physical"),
        "supply a current guarded selector within the query budget"
    );
    for pattern in [
        r"<urn:grapharch:snapshot:[^>]+>",
        r"<urn:grapharch:pinned-contribution:[^>]+>",
        r"<urn:grapharch:scope-plan:[^>]+>",
    ] {
        guarded = guarded.replace(
            &context(&guarded, pattern)?,
            &context(&old, pattern)?,
        );
    }
    let revision = context(&old, r"<urn:grapharch:snapshot:[^>]+>")?;
    let scope = context(&old, r"<urn:grapharch:scope-plan:[^>]+>")?;
    let scope_snapshot =
        context(&old, r"<urn:grapharch:pinned-contribution:[^>]+>")?;
    let dataset = Dataset::memory()?;
    let started = Instant::now();
    dataset.import(archive.into()).await?;
    eprintln!("real KG imported in {:?}", started.elapsed());
    // Confirm that the capture contains this scope, rather than accepting an
    // accidentally empty result caused by mismatched snapshot identifiers.
    let members = Dataset::logical_pattern(
        &oxrdf::NamedNode::new(scope_snapshot.trim_matches(['<', '>']))?,
        &format!(
            "{scope} <https://ekgf.org/ontology/grapharch#member> ?member"
        ),
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT ?member WHERE {{ {members} }} LIMIT 1"
        ))
        .await?;
    ensure!(
        rows.next().await?.is_some(),
        "capture lacks the selected scope"
    );
    drop(rows);
    let baseline = assets(&dataset, format!("SELECT DISTINCT ?asset WHERE {{ GRAPH ?graph {{ ?plan <https://ekgf.org/ontology/grapharch#scopePlan> {scope} ; <https://ekgf.org/ontology/grapharch#inputRevision> {revision} ; <https://ekgf.org/ontology/grapharch#asset> ?asset }} }}")).await?;
    assert_eq!(
        baseline.len(),
        expected,
        "captured plan asset count"
    );
    let started = Instant::now();
    let selected = assets(&dataset, guarded).await?;
    eprintln!(
        "guarded selector: {:?}, {} assets",
        started.elapsed(),
        selected.len()
    );
    assert_eq!(
        selected, baseline,
        "selection must equal the captured plan exactly"
    );
    Ok(())
}
