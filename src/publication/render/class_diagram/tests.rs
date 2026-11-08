use {
    super::*,
    crate::publication::document::class_diagram::{Diagram, Edge},
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
fn textual_relationships_are_a_full_width_accessible_table() {
    let output = html(&retained("#class")).unwrap();
    let parsed = scraper::Html::parse_fragment(&output);
    let details = parsed
        .select(&scraper::Selector::parse("details.owl-relationships").unwrap())
        .next()
        .expect("relationship disclosure");
    assert!(!details.ancestors().any(|node| {
        scraper::ElementRef::wrap(node)
            .is_some_and(|element| element.value().name() == "figure")
    }));
    let table = details
        .select(&scraper::Selector::parse("table").unwrap())
        .next()
        .expect("three-column relationship table");
    assert_eq!(
        table.value().attr("aria-label"),
        Some("Asserted OWL relationships")
    );
    assert_eq!(
        table
            .select(
                &scraper::Selector::parse("thead.sr-only th[scope=col]")
                    .unwrap()
            )
            .count(),
        3
    );
    assert_eq!(
        table
            .select(&scraper::Selector::parse("tbody tr td").unwrap())
            .count(),
        3
    );
    assert!(table.text().collect::<String>().contains("rdfs:subClassOf"));
    assert_eq!(
        details
            .select(&scraper::Selector::parse("ul,li").unwrap())
            .count(),
        0
    );
    assert!(!output.contains(" — ") && !output.contains(" → "));
}
#[test]
fn tall_pdf_drawings_have_a_physical_height_budget_and_text_links() {
    let output = typst(&retained("#class")).unwrap();
    assert!(output.contains("calc.min(size.width * 1.0, 210pt)"));
    assert!(output.contains("#link(label(\"class\"))"));
    assert!(!output.contains("grapharch-node"));
}
#[test]
fn phone_diagrams_keep_readable_intrinsic_width_inside_a_scroll_region() {
    let output = html(&retained("#class")).unwrap();
    assert!(output.contains("min-width:340px"));
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
    assert!(output.contains("SHACL S1") && output.contains("SHACL S2"));
    assert!(output.contains("#link(label(\"one\"))"));
}

#[test]
fn shape_markers_link_to_named_sources_in_captions_and_text() {
    let output = html(&shape_only()).unwrap();
    let parsed = scraper::Html::parse_fragment(&output);
    let markers: Vec<_> = parsed
        .select(&scraper::Selector::parse("figcaption .shape-marker").unwrap())
        .collect();
    assert_eq!(markers.len(), 2);
    for (index, marker) in markers.iter().enumerate() {
        assert_eq!(
            marker.text().collect::<String>(),
            format!("S{}", index + 1)
        );
        assert_eq!(
            marker.value().attr("href"),
            Some(if index == 0 { "#one" } else { "#two" })
        );
        let name = if index == 0 {
            "First & specific shape"
        } else {
            "Second shape"
        };
        assert_eq!(marker.value().attr("title"), Some(name));
        assert!(marker.value().attr("aria-label").unwrap().contains(name));
    }
    let details = parsed
        .select(&scraper::Selector::parse("details.owl-relationships").unwrap())
        .next()
        .unwrap()
        .text()
        .collect::<String>();
    assert!(details.contains("SHACL S1") && details.contains("SHACL S2"));
}
