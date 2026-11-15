use super::*;

#[derive(serde::Deserialize)]
struct Retained {
    source: String,
    svg:    String,
}
fn retained(body: &str) -> Result<Retained> {
    let json = body
        .split("```grapharch-class-diagram\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    Ok(serde_json::from_str(json)?)
}
fn assert_no_property_box(source: &str) {
    for raw in [
        "property>>",
        "rdfs:\\ndomain",
        "rdfs:\\nrange",
        "line.dotted",
    ] {
        assert!(
            !source.contains(raw),
            "unexpected {raw}: {source}"
        );
    }
}
fn associations(source: &str, label: &str) -> usize {
    source
        .lines()
        .filter(|line| {
            line.starts_with('n') &&
                line.contains(" --> ") &&
                line.ends_with(label)
        })
        .count()
}

#[tokio::test]
async fn object_property_with_an_undeclared_domain_class_is_one_labelled_edge()
-> Result<()> {
    // DPROD: dcat:Dataset is not declared as an owl:Class in the inputs.
    let body = diagram(
        r#"
ex:Core a owl:Class ; rdfs:label "information sensitivity classification" .
ex:classification a owl:ObjectProperty ;
  rdfs:label "information sensitivity classification" ;
  rdfs:domain ex:Dataset ; rdfs:range ex:Core .
"#,
    )
    .await?;
    let drawing = retained(&body)?;
    assert_no_property_box(&drawing.source);
    assert_eq!(
        associations(
            &drawing.source,
            ": information sensitivity\\nclassification"
        ),
        1,
        "{}",
        drawing.source
    );
    assert!(drawing.svg.contains("./grapharch-node/"));
    Ok(())
}

#[tokio::test]
async fn multiple_domains_and_ranges_draw_one_edge_per_declared_pair()
-> Result<()> {
    let body = diagram(
        r#"
ex:Core a owl:Class . ex:Other a owl:Class .
ex:Target a owl:Class . ex:Second a owl:Class .
ex:p a owl:ObjectProperty ; rdfs:domain ex:Core, ex:Other ;
  rdfs:range ex:Target, ex:Second .
"#,
    )
    .await?;
    let drawing = retained(&body)?;
    assert_no_property_box(&drawing.source);
    assert_eq!(
        associations(&drawing.source, ": ex:\\np"),
        4,
        "{}",
        drawing.source
    );
    Ok(())
}

#[tokio::test]
async fn datatype_property_is_an_attribute_of_every_declared_domain()
-> Result<()> {
    let body = diagram(
        r#"
ex:Core a owl:Class .
ex:title a owl:DatatypeProperty ; rdfs:domain ex:Core, ex:Dataset ;
  rdfs:range <http://www.w3.org/2001/XMLSchema#string>,
             <http://www.w3.org/2001/XMLSchema#token> .
"#,
    )
    .await?;
    let drawing = retained(&body)?;
    assert_no_property_box(&drawing.source);
    assert!(
        drawing.source.contains("\" as n3 <<Resource>> "),
        "an undeclared domain still owns its attributes: {}",
        drawing.source
    );
    assert_eq!(
        drawing
            .source
            .matches("{field} ex:title : string and token [[[")
            .count(),
        2,
        "{}",
        drawing.source
    );
    Ok(())
}

#[tokio::test]
async fn range_only_properties_with_several_ranges_share_the_placeholder()
-> Result<()> {
    let body = diagram(
        "ex:Core a owl:Class . ex:Other a owl:Class . ex:p a \
         owl:ObjectProperty ; rdfs:range ex:Core, ex:Other .",
    )
    .await?;
    let drawing = retained(&body)?;
    assert_eq!(
        drawing.source.matches("class \"rdfs:Resource\"").count(),
        1
    );
    assert_eq!(
        drawing
            .source
            .lines()
            .filter(|l| l.starts_with("unknownDomain ..> "))
            .count(),
        2,
        "{}",
        drawing.source
    );
    assert!(!drawing.source.contains("property>>"));
    Ok(())
}

#[tokio::test]
async fn reserved_plantuml_characters_in_labels_render_literally() -> Result<()>
{
    let label = r#"Odd (a) <b> "c" «d» [e] {f} \g ~h &i #j"#;
    let body = diagram(&format!(
        r#"
ex:Core a owl:Class ; rdfs:label "{}" .
ex:Target a owl:Class ; rdfs:label "end)" .
ex:p a owl:ObjectProperty ; rdfs:label "links (to)" ; rdfs:domain ex:Core ;
  rdfs:range ex:Target .
"#,
        label.replace('\\', "\\\\").replace('"', "\\\"")
    ))
    .await?;
    let drawing = retained(&body)?;
    for reserved in ['(', ')', '<', '>', '"', '«', '»', '[', ']', '{', '}'] {
        assert!(
            !drawing
                .source
                .lines()
                .filter(|l| l.contains("Odd") || l.contains("end"))
                .any(|l| {
                    l.split_once(" as ").is_some_and(|(name, _)| {
                        name.trim_start_matches("class ")
                            .trim_matches('"')
                            .contains(reserved)
                    })
                }),
            "label text must be entity-encoded, not PlantUML syntax: {}",
            drawing.source
        );
    }
    let svg = roxmltree::Document::parse(&drawing.svg)?;
    let text: String = svg
        .descendants()
        .filter(|n| n.has_tag_name("text"))
        .filter_map(|n| n.text())
        .collect();
    assert!(
        !text.contains("U+"),
        "stray template marker: {text}"
    );
    for expected in [
        "Odd (a) <b> \"c\"",
        "«d» [e] {f} \\g ~h &i #j",
        "end)",
        "links (to)",
    ] {
        assert!(
            text.replace(' ', "").contains(&expected.replace(' ', "")),
            "missing {expected}: {text}"
        );
    }
    Ok(())
}
