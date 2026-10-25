use {
    crate::publication::native_html,
    anyhow::{Result, ensure},
    scraper::{ElementRef, Html, Selector},
    url::Url,
};

pub(super) struct Parsed {
    pub header:   super::header::Header,
    pub sections: Vec<Section>,
}

pub(super) struct Section {
    pub id:         String,
    pub heading_id: Option<String>,
    pub title:      String,
    pub kind:       &'static str,
    pub body:       String,
    pub original:   String,
    pub things:     Vec<Thing>,
}

pub(super) struct Thing {
    pub id:    String,
    pub title: String,
    pub class: &'static str,
}

pub(super) fn parse(source: &str, base: &Url) -> Result<Parsed> {
    let document = Html::parse_document(source);
    let head =
        document
            .select(&selector("div.head")?)
            .next()
            .ok_or_else(|| {
                anyhow::anyhow!("published W3C document requires div.head")
            })?;
    let header = super::header::parse(head, base)?;
    let mut sections = Vec::new();
    for element in document.select(&selector("body > section")?) {
        let id = element
            .value()
            .attr("id")
            .ok_or_else(|| anyhow::anyhow!("published section requires id"))?;
        if id == "toc" {
            continue;
        }
        ensure!(
            sections.len() < 256,
            "published specification exceeds 256 top-level sections"
        );
        let mut heading = None;
        let mut heading_wrapper = None;
        for child in element.children().filter_map(ElementRef::wrap) {
            if child.value().name() == "h2" {
                ensure!(
                    heading.is_none(),
                    "section {id} has ambiguous headings"
                );
                heading = Some(child);
                heading_wrapper = Some(child.id());
            } else if child
                .value()
                .classes()
                .any(|class| class == "header-wrapper") &&
                let Some(h2) = child
                    .children()
                    .filter_map(ElementRef::wrap)
                    .find(|node| node.value().name() == "h2")
            {
                ensure!(
                    heading.is_none(),
                    "section {id} has ambiguous headings"
                );
                heading = Some(h2);
                heading_wrapper = Some(child.id());
            }
        }
        let heading =
            heading.ok_or_else(|| anyhow::anyhow!("section {id} has no h2"))?;
        let title = native_html::title(heading);
        let original = element.inner_html();
        let mut body = String::new();
        for node in element.children() {
            if let Some(child) = ElementRef::wrap(node) {
                if Some(child.id()) != heading_wrapper {
                    body.push_str(&child.html());
                }
            } else if let Some(text) = node.value().as_text() {
                body.push_str(text);
            }
        }
        let body =
            native_html::read(&body, Some(base), &mut |_| Ok(None))?.html;
        let kind = match id {
            "abstract" => "abstract",
            "sotd" => "status",
            _ if element.value().classes().any(|c| c == "appendix") => {
                "appendix"
            },
            _ => "section",
        };
        let mut things = vec![Thing {
            id:    id.to_owned(),
            title: title.clone(),
            class: "Section",
        }];
        for thing in element.select(&selector(
            "section[id], dfn[id], div.example, aside.example, \
             figure.example, dt[id]",
        )?) {
            let class = match thing.value().name() {
                "section" => "Section",
                "dfn" => "DefinedTerm",
                "dt" => "BibliographicEntry",
                _ => "Example",
            };
            let title = if class == "Section" {
                let header =
                    thing.select(&selector("h3,h4,h5,h6")?).next().ok_or_else(
                        || anyhow::anyhow!("nested section requires a heading"),
                    )?;
                native_html::title(header)
            } else if class == "Example" {
                thing
                    .select(&selector(".example-title")?)
                    .next()
                    .map(native_html::title)
                    .unwrap_or_else(|| "Example".into())
            } else {
                native_html::title(thing)
            };
            things.push(Thing {
                id: if class == "Example" {
                    native_html::example_id(thing)
                } else {
                    thing.value().attr("id").unwrap().to_owned()
                },
                title,
                class,
            });
        }
        sections.push(Section {
            id: id.to_owned(),
            heading_id: heading.value().attr("id").map(str::to_owned),
            title,
            kind,
            body,
            original,
            things,
        });
    }
    ensure!(
        !sections.is_empty(),
        "published W3C document contains no supported sections"
    );
    Ok(Parsed { header, sections })
}

fn selector(css: &str) -> Result<Selector> {
    Selector::parse(css).map_err(|error| {
        anyhow::anyhow!("invalid collector selector {css}: {error}")
    })
}
