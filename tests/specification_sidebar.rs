use grapharch::publication::{render::Publisher, showcases};

#[tokio::test]
async fn specification_has_an_accessible_sidebar_resize_control()
-> anyhow::Result<()> {
    let (dataset, plans) = showcases::specification(std::path::Path::new(
        "examples/specification/project.ttl",
    ))
    .await?;
    let output = tempfile::tempdir()?;
    showcases::publish(&Publisher::new(dataset), &plans, output.path()).await?;
    let html = std::fs::read_to_string(
        output.path().join("specification/index.html"),
    )?;
    let document = scraper::Html::parse_document(&html);
    let selector = scraper::Selector::parse("[role=separator][aria-controls=toc][aria-orientation=vertical][tabindex='0']").unwrap();
    assert_eq!(
        document.select(&selector).count(),
        1,
        "provide one keyboard-accessible sidebar resize handle"
    );
    assert!(
        html.contains("pointerdown"),
        "drag behavior must ship with the generated page"
    );
    assert!(
        html.contains("ArrowRight"),
        "keyboard resizing must ship with the generated page"
    );
    let pdf_icon = scraper::Selector::parse(
        "a.pdf-download[href='document.pdf'] svg text",
    )
    .unwrap();
    assert!(
        document
            .select(&pdf_icon)
            .any(|node| node.text().collect::<String>() == "PDF"),
        "PDF download must have a recognizable PDF file icon"
    );
    Ok(())
}
