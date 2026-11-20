use {
    super::*,
    crate::publication::document::class_diagram::{Diagram, Edge},
};

fn node(id: &str, kind: NodeKind) -> Node {
    Node {
        id: id.into(),
        label: id.into(),
        kind,
        link: None,
        imported: false,
        datatype: false,
    }
}

fn projection() -> Projection {
    let focus = node("urn:a:Focus", NodeKind::Class);
    let mut imported = node("urn:b:Imported", NodeKind::Class);
    imported.imported = true;
    let mut datatype = node(
        "http://www.w3.org/2001/XMLSchema#string",
        NodeKind::Resource,
    );
    datatype.datatype = true;
    let restriction = node("expression-1", NodeKind::OwlRestriction);
    let edge = |to: &Node| {
        Edge {
            from:      focus.clone(),
            relation:  "rdfs:subClassOf".into(),
            predicate: None,
            to:        to.clone(),
        }
    };
    // Entity names follow identity order: n0 expression, n1 xsd:string,
    // n2 Focus, n3 Imported.
    Projection {
        diagram: Diagram {
            edges: vec![edge(&imported), edge(&datatype), edge(&restriction)],
            focus,
            shapes: Vec::new(),
            limited: false,
            context: Default::default(),
        },
        source:  String::new(),
        engine:  "PlantUML version fixture".into(),
        svg:     String::new(),
    }
}

const SVG: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\"><g \
     font-family=\"sans-serif\"><g class=\"entity\" \
     data-qualified-name=\"n0\"><rect fill=\"#FFF\"/><text fill=\"#000\" \
     font-family=\"Arial\">r</text></g><g class=\"entity\" \
     data-qualified-name=\"n1\"><rect fill=\"#FFF\"/></g><g class=\"entity\" \
     data-qualified-name=\"n2\"><rect fill=\"#E9EDFF\"/></g><g \
     class=\"entity\" data-qualified-name=\"n3\"><rect fill=\"#FFF\"/></g><g \
     class=\"entity\" data-qualified-name=\"unknownDomain\"><rect \
     fill=\"#FFF\"/></g></g></svg>";

#[test]
fn every_box_takes_the_fill_of_its_role_and_text_the_theme_font() {
    let typst = ThemedSvg::new(&projection(), SVG).unwrap().typst(SVG);
    let order = [
        "ga-theme.diagram_font",
        "ga-theme.diagram_expression_fill.to-hex()",
        "ga-theme.diagram_font",
        "ga-theme.diagram_datatype_fill.to-hex()",
        "ga-theme.diagram_focus_fill.to-hex()",
        "ga-theme.diagram_imported_class_fill.to-hex()",
    ];
    let mut cursor = 0;
    for value in order {
        let found = typst[cursor..].find(value).unwrap_or_else(|| {
            panic!("{value} missing after {cursor} in {typst}")
        });
        cursor += found + value.len();
    }
    // Text colour and the unattributed placeholder box keep their fills.
    assert!(typst.contains(r##"<text fill=\"#000\""##));
    assert!(typst.contains(r##"unknownDomain\"><rect fill=\"#FFF\""##));
    assert!(!typst.contains("Arial") && !typst.contains("sans-serif"));
    assert!(typst.ends_with(",).join()"));
}

#[test]
fn roles_distinguish_imported_classes_datatypes_and_properties() {
    let focus = node("urn:a:Focus", NodeKind::Class);
    let mut imported = node("urn:b:C", NodeKind::Resource);
    imported.imported = true;
    assert_eq!(Role::of(&imported, &focus), Role::ImportedClass);
    let mut datatype = imported.clone();
    datatype.datatype = true;
    assert_eq!(Role::of(&datatype, &focus), Role::Datatype);
    assert_eq!(
        Role::of(&node("urn:a:p", NodeKind::ObjectProperty), &focus),
        Role::ObjectProperty
    );
    assert_eq!(
        Role::of(
            &node("urn:a:d", NodeKind::DatatypeProperty),
            &focus
        ),
        Role::DatatypeProperty
    );
    assert_eq!(Role::of(&focus, &focus), Role::Focus);
}
