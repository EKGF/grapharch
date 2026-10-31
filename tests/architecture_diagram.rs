#[test]
fn architecture_diagram_remains_legible_in_a_documentation_column() {
    let markdown = include_str!("../docs/other/architecture/README.md");
    let code = markdown
        .split("```mermaid\n")
        .nth(1)
        .expect("architecture diagram")
        .split("```")
        .next()
        .unwrap();
    let parsed = mermaid_rs_renderer::parse_mermaid_strict(code).unwrap();
    let options = mermaid_rs_renderer::RenderOptions::default();
    let layout = mermaid_rs_renderer::compute_layout(
        &parsed.graph,
        &options.theme,
        &options.layout,
    );
    let svg = mermaid_rs_renderer::render_svg(
        &layout,
        &options.theme,
        &options.layout,
    );
    let document = scraper::Html::parse_fragment(&svg);
    let image = document
        .select(&scraper::Selector::parse("svg").unwrap())
        .next()
        .unwrap();
    let viewbox: Vec<f64> = image
        .value()
        .attr("viewBox")
        .or_else(|| image.value().attr("viewbox"))
        .unwrap()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    let text_selector = scraper::Selector::parse("text").unwrap();
    let mut labels = 0;
    for text in document.select(&text_selector) {
        let font: f64 = text
            .value()
            .attr("font-size")
            .expect("explicit SVG font size")
            .trim_end_matches("px")
            .parse()
            .unwrap();
        let displayed = font * (560.0 / viewbox[2]).min(1.0);
        assert!(
            displayed >= 14.0,
            "architecture label would shrink to {displayed:.1}px at 560px \
             column width (SVG width {})",
            viewbox[2]
        );
        labels += 1;
    }
    assert!(
        labels >= 6,
        "diagram retains its stages and shared dataset"
    );
}
