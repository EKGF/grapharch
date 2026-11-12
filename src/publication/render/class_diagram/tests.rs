use {
    super::*,
    crate::publication::document::class_diagram::{Diagram, Edge, Node},
};
fn retained(link: &str) -> String {
    let node = Node {
        id:    "urn:test:class".into(),
        label: "Class".into(),
        kind:  "Class".into(),
        link:  Some(link.into()),
    };
    serde_json::to_string(&Projection {
        diagram: Diagram {
            focus:   node.clone(),
            shapes:  Vec::new(),
            limited: false,
            edges:   vec![Edge {
                predicate: None,
                from:      node.clone(),
                relation:  "rdfs:subClassOf".into(),
                to:        node,
            }],
        },
        source:  "@startuml\n@enduml\n".into(),
        engine:  "PlantUML version fixture".into(),
        svg:     "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"400\" \
                  height=\"800\" viewBox=\"0 0 400 800\"><a \
                  href=\"./grapharch-node/n0\"><rect width=\"400\" \
                  height=\"800\"/></a></svg>"
            .into(),
    })
    .unwrap()
}
#[test]
fn diagram_has_accessible_description_without_duplicate_sections() {
    let output = html(&retained("#class")).unwrap();
    let parsed = scraper::Html::parse_fragment(&output);
    assert_eq!(
        parsed
            .select(&scraper::Selector::parse("details, table").unwrap())
            .count(),
        0
    );
    let description = parsed
        .select(&scraper::Selector::parse("svg desc").unwrap())
        .next()
        .expect("accessible SVG description");
    assert!(
        description
            .text()
            .collect::<String>()
            .contains("Class rdfs:subClassOf Class")
    );
    assert!(!output.contains("Relationships as text"));
}
#[test]
fn tall_pdf_drawings_have_a_physical_height_budget_and_text_links() {
    let output = typst(&retained("#class")).unwrap();
    assert!(output.contains("calc.min(size.width * 1.0, 210pt)"));
    assert!(!output.contains("#ga-table") && !output.contains("#par[SHACL"));
    assert!(!output.contains("grapharch-node"));
}
#[test]
fn pdf_diagram_note_stays_inside_the_unbreakable_figure() {
    let output = typst(&retained("#class")).unwrap();
    assert!(output.starts_with("#block(breakable: false)["));
    assert!(output.ends_with("\n]\n"));
    assert!(
        !output.contains("))]\n#align"),
        "note must not escape the figure block"
    );
}
#[test]
fn phone_diagrams_keep_readable_intrinsic_width_inside_a_scroll_region() {
    let output = html(&retained("#class")).unwrap();
    assert!(output.contains("width:400px;max-width:none"));
    assert!(output.contains("class=\"class-diagram-scroll\""));
    assert!(output.contains("overflow-x:auto"));
    assert!(output.contains("tabindex=\"0\""));
}
#[test]
fn retained_svg_links_reject_executable_destinations() {
    assert!(html(&retained("javascript:alert(1)")).is_err());
    assert!(html(&retained("//evil.example")).is_err());
    assert!(
        html(&retained("#class"))
            .unwrap()
            .contains("href=\"#class\"")
    );
}

fn shape_only() -> String {
    let mut projection = Projection::parse(&retained("#class")).unwrap();
    projection.diagram.edges.clear();
    for (id, label) in
        [("one", "First & specific shape"), ("two", "Second shape")]
    {
        let source = format!(
            r##"{{"class":{{"id":"urn:test:class","label":"Class","kind":"Class","link":"#class"}},"source":{{"id":"urn:test:{id}","label":"{label}","kind":"Resource","link":"#{id}"}},"fields":[],"omitted":0}}"##
        );
        projection
            .diagram
            .shapes
            .push(serde_json::from_str(&source).unwrap());
    }
    serde_json::to_string(&projection).unwrap()
}

#[test]
fn shape_only_pdf_uses_available_width_with_its_height_bound() {
    let output = typst(&shape_only()).unwrap();
    assert!(output.contains("calc.min(size.width * 1.0, 210pt)"));
    assert!(!output.contains("SHACL constraints"));
    assert!(!output.contains("#par["));
}

#[test]
fn shape_sources_are_retained_without_visible_duplicate_sections() {
    let source = shape_only();
    let output = html(&source).unwrap();
    assert!(!output.contains("<details") && !output.contains("SHACL"));
    assert_eq!(
        Projection::parse(&source).unwrap().diagram.shapes.len(),
        2
    );
}

#[test]
fn compact_pdf_diagrams_never_enlarge_beyond_intrinsic_readable_size() {
    let mut projection = Projection::parse(&shape_only()).unwrap();
    projection.svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 \
                      400 200\"><rect width=\"400\" height=\"200\"/></svg>"
        .into();
    let output = typst(&serde_json::to_string(&projection).unwrap()).unwrap();
    assert!(output.contains("calc.min(size.width * 1.0, 300pt)"));
}

#[tokio::test]
async fn pdf_diagram_links_survive_native_compilation() -> anyhow::Result<()> {
    for destination in [
        "https://example.org/diagram-only-target",
        "./section-other-book.html#class",
    ] {
        diagram_link_survives_native_compilation(destination).await?;
    }
    Ok(())
}

async fn diagram_link_survives_native_compilation(
    destination: &str,
) -> anyhow::Result<()> {
    let diagram = typst(&retained(destination))?;
    let root = tempfile::tempdir()?;
    tokio::fs::write(
        root.path().join("main.typ"),
        format!(
            "#let ga-diagram-note = true\n#set page(width: 500pt, height: \
             600pt, margin: 25pt)\n{diagram}\n#pagebreak()\n= Interpretation \
             <grapharch-diagram-note>\n"
        ),
    )
    .await?;
    let pdf = crate::publication::render::engine::compile(
        root.path().to_owned(),
        crate::publication::render::engine::Assets::load().await?,
        std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
    )
    .await?;
    if let Ok(destination) = std::env::var("GRAPHARCH_DIAGRAM_LINK_REVIEW") {
        tokio::fs::write(destination, &pdf).await?;
    }
    assert!(
        String::from_utf8_lossy(&pdf).contains("/Rect[25 155 235 575]"),
        "diagram annotation bounds must match the unchanged 210pt by 420pt \
         image at the page margin"
    );
    assert!(
        String::from_utf8_lossy(&pdf).contains(destination),
        "a diagram-only destination must survive as a PDF link annotation"
    );
    Ok(())
}
