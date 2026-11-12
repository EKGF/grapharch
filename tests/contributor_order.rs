use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        showcases,
    },
    scraper::{Html, Selector},
};

const PEOPLE: &str = r#"
ex:p1 dct:title "Jacobus Geluk, Initiator" .
ex:p2 dct:title "Marcel Fröhlich" .
ex:p3 dct:title "Mathias Autrata" .
ex:p4 dct:title "Tony Seale, Chair" .
ex:p5 dct:title "Ben Whittam Smith" ; <https://schema.org/familyName> "Whittam Smith" .
ex:p6 dct:title "Zara Voss" .
ex:p7 dct:title "Prince" .
"#;
const NAMES: [&str; 7] = [
    "Mathias Autrata",
    "Marcel Fröhlich",
    "Jacobus Geluk, Initiator",
    "Prince",
    "Tony Seale, Chair",
    "Zara Voss",
    "Ben Whittam Smith",
];

async fn render(extra: &str) -> Result<(String, String)> {
    let input = tempfile::tempdir()?;
    let fixture = include_str!("../examples/specification/project.ttl")
        .replace(
            "ga:editor ex:alex ;",
            "ga:editor ex:p1, ex:p2, ex:p3, ex:p4, ex:p5, ex:p6, ex:p7 ; \
             ga:author ex:p1, ex:p2, ex:p3, ex:p4, ex:p5, ex:p6, ex:p7 ;",
        );
    let source = input.path().join("project.ttl");
    tokio::fs::write(&source, format!("{fixture}\n{PEOPLE}\n{extra}")).await?;
    let (dataset, plans) = showcases::specification(&source).await?;
    let export = tempfile::tempdir()?;
    dataset.export(export.path().join("graph.nq")).await?;
    input.close()?;
    let dataset = Dataset::memory()?;
    dataset.import(export.path().join("graph.nq")).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&plans[0], output.path())
        .await?
    else {
        panic!("publication required")
    };
    Ok((
        tokio::fs::read_to_string(publication.directory.join("index.html"))
            .await?,
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?,
    ))
}

fn role_names(html: &str, heading: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let terms = Selector::parse("header dt").unwrap();
    let term = doc
        .select(&terms)
        .find(|term| term.text().collect::<String>() == heading)
        .unwrap();
    let mut names = Vec::new();
    let mut next = term.next_sibling();
    while let Some(node) = next {
        next = node.next_sibling();
        if let Some(element) = scraper::ElementRef::wrap(node) {
            if element.value().name() == "dt" {
                break;
            }
            if element.value().name() == "dd" {
                names.push(element.text().collect());
            }
        }
    }
    names
}

#[tokio::test]
async fn contributors_default_to_surnames_in_html_and_pdf_from_restored_kg()
-> Result<()> {
    let (html, typst) = render("").await?;
    assert_eq!(role_names(&html, "Editors:"), NAMES);
    assert_eq!(role_names(&html, "Authors:"), NAMES);
    let authors = NAMES
        .iter()
        .map(|name| format!("\"{name}\", "))
        .collect::<String>();
    assert!(
        typst.contains(&format!("author: ({authors})")),
        "PDF author metadata lists each author in the same order"
    );
    Ok(())
}

#[tokio::test]
async fn explicit_source_order_overrides_surnames() -> Result<()> {
    let (html, _) = render(
        "ex:p1 ga:order 0 . ex:p2 ga:order 1 . ex:p3 ga:order 2 . ex:p4 \
         ga:order 3 . ex:p5 ga:order 4 . ex:p6 ga:order 5 . ex:p7 ga:order 6 .",
    )
    .await?;
    assert_eq!(
        role_names(&html, "Editors:")[0],
        "Jacobus Geluk, Initiator"
    );
    assert_eq!(
        role_names(&html, "Authors:")[0],
        "Jacobus Geluk, Initiator"
    );
    Ok(())
}
