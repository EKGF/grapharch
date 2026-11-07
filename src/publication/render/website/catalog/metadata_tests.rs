use {
    super::Catalog,
    crate::publication::{render::model::Plan, website},
    anyhow::Result,
    std::sync::atomic::Ordering,
};
#[tokio::test]
async fn metadata_queries_do_not_multiply_by_each_fragment_property()
-> Result<()> {
    let source = tempfile::tempdir()?;
    for name in ["index", "guide"] {
        let mut body = format!("# {name}\n\nIntroduction.\n");
        for index in 0..25 {
            body.push_str(&format!(
                "\n## Section {index}\n\nSource-owned section {index}.\n"
            ));
        }
        tokio::fs::write(source.path().join(format!("{name}.md")), body)
            .await?;
    }
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, r#"@prefix ga:<https://ekgf.org/ontology/grapharch#>. <urn:p> a ga:Project;ga:target <urn:t>. <urn:t> a ga:Target;ga:renderer "website";ga:destination "site";ga:scope <urn:s>;ga:profile <urn:f>. <urn:s> a ga:Scope. <urn:f> a ga:Profile."#).await?;
    let built =
        website::collect(source.path(), &definitions, "urn:p", "urn:t", 2)
            .await?;
    let plan = Plan::read(built.dataset.clone(), built.plan).await?;
    let mut entries = plan.entries().await?;
    let mut fragments = 0;
    while entries.next().await?.is_some() {
        fragments += 1;
    }
    let before = built.dataset.select_calls.load(Ordering::Relaxed);
    let catalog = Catalog::read(&plan).await?;
    let calls = built.dataset.select_calls.load(Ordering::Relaxed) - before;
    assert_eq!(catalog.pages.len(), 2);
    // Allow two metadata queries per selected fragment plus generous fixed
    // per-page/setup overhead; separate queries for each property exceed this.
    assert!(
        calls <= fragments * 2 + 2 * 20 + 8,
        "Catalog issued {calls} SELECTs for {fragments} fragments"
    );
    assert!(
        catalog
            .pages
            .values()
            .all(|page| page.headings.contains("section-24"))
    );
    Ok(())
}
