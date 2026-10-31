use {
    scraper::{Html, Selector},
    std::path::Path,
};

pub async fn assert_restored_metadata(
    html: &str,
    directory: &Path,
) -> anyhow::Result<()> {
    let document = Html::parse_document(html);
    let dt = Selector::parse("header dt").unwrap();
    let anchors = Selector::parse("a[href]").unwrap();
    let mut rows = std::collections::BTreeMap::new();
    for heading in document.select(&dt) {
        let title = heading.text().collect::<String>();
        let mut values = Vec::new();
        let mut sibling = heading.next_sibling();
        while let Some(node) = sibling {
            sibling = node.next_sibling();
            let Some(element) = scraper::ElementRef::wrap(node) else {
                continue
            };
            if element.value().name() == "dt" {
                break;
            }
            if element.value().name() == "dd" {
                values.push(
                    element
                        .select(&anchors)
                        .filter_map(|a| a.value().attr("href"))
                        .map(str::to_owned)
                        .collect::<Vec<_>>(),
                );
            }
        }
        rows.insert(title, values);
    }
    assert_eq!(rows["Editors:"].len(), 6);
    assert_eq!(rows["Authors:"].len(), 9);
    assert_eq!(rows["History:"][0], [
        "https://github.com/EKGF/dprod/commits/develop/"
    ]);
    assert_eq!(rows["Feedback:"][0], [
        "https://github.com/EKGF/dprod",
        "https://github.com/EKGF/dprod/pulls",
        "https://github.com/EKGF/dprod/issues/new",
        "https://github.com/EKGF/dprod/issues"
    ]);
    let downloads = &rows["Download as RDF:"][0];
    assert_eq!(downloads.len(), 12);
    assert!(
        downloads
            .iter()
            .all(|url| url.starts_with("https://ekgf.org/dprod/spec/develop/"))
    );
    let logo = document
        .select(&Selector::parse("header img.product-logo").unwrap())
        .next()
        .expect("retained product logo");
    assert!(!logo.value().attr("alt").unwrap_or_default().is_empty());
    let route = logo.value().attr("src").expect("logo route");
    assert!(
        !route.contains("://"),
        "logo must use retained local asset"
    );
    assert!(!tokio::fs::read(directory.join(route)).await?.is_empty());
    let css = tokio::fs::read_to_string(directory.join("style.css")).await?;
    assert!(css.contains(".product-logo{float:right;width:100px;"));
    Ok(())
}
