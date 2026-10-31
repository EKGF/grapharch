use {
    grapharch::publication::{render::Publisher, showcases},
    scraper::{Html, Selector},
    std::path::Path,
};

#[tokio::test]
async fn compact_contents_controls_keep_accessible_names_and_grouping()
-> anyhow::Result<()> {
    let (dataset, plans) = showcases::specification(Path::new(
        "examples/specification/project.ttl",
    ))
    .await?;
    let output = tempfile::tempdir()?;
    let publications =
        showcases::publish(&Publisher::new(dataset), &plans, output.path())
            .await?;
    let directory = &publications[0].directory;
    let html = tokio::fs::read_to_string(directory.join("index.html")).await?;
    let document = Html::parse_document(&html);
    let group = document
        .select(
            &Selector::parse(
                ".toc-actions[role='group'][aria-label='Contents sections']",
            )
            .unwrap(),
        )
        .next()
        .expect("the two contents utilities need one named semantic group");
    for (attribute, label) in [
        ("data-toc-collapse", "Collapse all sections"),
        ("data-toc-expand", "Expand all sections"),
    ] {
        let selector =
            Selector::parse(&format!("button[{attribute}]")).unwrap();
        let button = group.select(&selector).next().expect("contents utility");
        assert_eq!(button.value().attr("type"), Some("button"));
        assert_eq!(button.value().attr("aria-label"), Some(label));
        assert_eq!(button.value().attr("aria-controls"), Some("toc"));
        assert!(!button.text().collect::<String>().trim().is_empty());
        assert_eq!(
            button
                .select(
                    &Selector::parse(
                        "svg[aria-hidden='true'][focusable='false']"
                    )
                    .unwrap()
                )
                .count(),
            1,
            "utility icons must not repeat their accessible button labels"
        );
    }
    let banner = document
        .select(&Selector::parse(".authority-banner").unwrap())
        .next()
        .unwrap();
    assert_eq!(
        banner.text().collect::<String>(),
        "EKGF Editor's Draft"
    );
    assert!(html.contains("button.addEventListener('click'"));
    if let Ok(destination) = std::env::var("GRAPHARCH_CONTROLS_REVIEW") {
        tokio::fs::create_dir_all(&destination).await?;
        for name in ["index.html", "style.css"] {
            tokio::fs::copy(
                directory.join(name),
                Path::new(&destination).join(name),
            )
            .await?;
        }
    }
    Ok(())
}
