use {
    crate::publication::native_html,
    anyhow::{Result, ensure},
    scraper::{ElementRef, Selector},
    url::Url,
};

pub(super) struct Header {
    pub title:          String,
    pub subtitle:       Option<String>,
    pub date:           String,
    pub status:         String,
    pub status_url:     Option<String>,
    pub modern:         bool,
    pub versions:       Vec<(String, String)>,
    pub editors:        Vec<Person>,
    pub authors:        Vec<Person>,
    pub former_editors: Vec<Person>,
    pub copyright:      Option<String>,
    pub rows:           Vec<Row>,
}

pub(super) struct Person {
    pub name:        String,
    pub affiliation: Option<String>,
    pub url:         Option<String>,
    pub note:        String,
    pub orcid:       Option<String>,
}

pub(super) struct Row {
    pub field:      String,
    pub label:      String,
    pub link_label: Option<String>,
    pub body:       String,
}

pub(super) fn parse(head: ElementRef<'_>, base: &Url) -> Result<Header> {
    let title = text(head, "h1")?;
    let subtitle = head
        .select(&selector("#subtitle")?)
        .next()
        .map(native_html::title);
    let modern_status = head.select(&selector("#w3c-state")?).next();
    let modern = modern_status.is_some();
    let state = match modern_status {
        Some(state) => state,
        None => {
            head.select(&selector("h2")?)
                .find(|h| h.select(&selector("time").unwrap()).next().is_some())
                .ok_or_else(|| {
                    anyhow::anyhow!("publication status heading missing")
                })?
        },
    };
    let date = state
        .select(&selector("time[datetime]")?)
        .next()
        .and_then(|time| time.value().attr("datetime"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "published W3C document requires a publication date"
            )
        })?
        .to_owned();
    let raw_status = native_html::title(state);
    let status = raw_status
        .strip_prefix("W3C ")
        .and_then(|s| {
            s.split_once(char::is_numeric)
                .map(|(s, _)| s.trim().to_owned())
        })
        .ok_or_else(|| anyhow::anyhow!("unsupported W3C publication status"))?;
    let status_url = state
        .select(&selector("a[href]")?)
        .next()
        .and_then(|a| a.value().attr("href"))
        .map(|url| base.join(url).map(|u| u.to_string()))
        .transpose()?;
    let mut header = Header {
        title,
        subtitle,
        date,
        status,
        status_url,
        modern,
        versions: Vec::new(),
        editors: Vec::new(),
        authors: Vec::new(),
        former_editors: Vec::new(),
        copyright: head
            .select(&selector(".copyright")?)
            .next()
            .map(native_html::title),
        rows: Vec::new(),
    };
    for dt in head.select(&selector("dt")?) {
        ensure!(
            header.rows.len() < 64,
            "published header exceeds 64 fields"
        );
        let label = native_html::title(dt);
        let field = match label.as_str() {
            "This version:" => "thisVersion",
            "Latest published version:" => "latestVersion",
            "Previous version:" => "previousVersion",
            "Latest editor's draft:" => "editorDraft",
            "Implementation report:" => "implementationReport",
            "Test suite:" => "testSuite",
            "Latest Recommendation:" => "latestRecommendation",
            "Editors:" | "Editor:" => "editor",
            "Authors:" | "Author:" => "author",
            "Former editors:" | "Former editor:" => "formerEditor",
            _ => "custom",
        };
        let mut bodies = Vec::new();
        let mut link_label = None;
        for dd in dt.next_siblings().filter_map(ElementRef::wrap) {
            if dd.value().name() == "dt" {
                break;
            }
            if dd.value().name() != "dd" {
                continue;
            }
            match field {
                "editor" | "author" | "formerEditor" => {
                    let person = person(dd, base)?;
                    match field {
                        "editor" => header.editors.push(person),
                        "author" => header.authors.push(person),
                        _ => header.former_editors.push(person),
                    }
                },
                "custom" => {
                    let body = native_html::read(
                        &dd.inner_html(),
                        Some(base),
                        &mut |_| Ok(None),
                    )?
                    .html;
                    bodies.push(format!(
                        "<div class=\"metadata-value\">{body}</div>"
                    ));
                },
                _ => {
                    ensure!(
                        link_label.is_none(),
                        "{label} has ambiguous URL values"
                    );
                    let anchor =
                        dd.select(&selector("a[href]")?).next().ok_or_else(
                            || anyhow::anyhow!("{label} requires a URL"),
                        )?;
                    let url = base
                        .join(anchor.value().attr("href").unwrap())?
                        .to_string();
                    link_label = Some(native_html::title(anchor));
                    header.versions.push((field.to_owned(), url));
                },
            }
        }
        header.rows.push(Row {
            field: field.to_owned(),
            label,
            link_label,
            body: bodies.join(""),
        });
    }
    ensure!(
        header.versions.iter().any(|(k, _)| k == "thisVersion") &&
            header.versions.iter().any(|(k, _)| k == "latestVersion"),
        "required publication version links missing"
    );
    Ok(header)
}

fn person(dd: ElementRef<'_>, base: &Url) -> Result<Person> {
    let name_node = dd
        .select(&selector(
            ".p-name:not(.orcid), [property='foaf:name']",
        )?)
        .next()
        .ok_or_else(|| anyhow::anyhow!("published person requires a name"))?;
    let name = native_html::title(name_node);
    let url = name_node
        .value()
        .attr("href")
        .map(|url| base.join(url).map(|u| u.to_string()))
        .transpose()?;
    let affiliation = dd
        .select(&selector(".p-org")?)
        .next()
        .map(native_html::title);
    let orcid = dd
        .select(&selector("a.orcid[href]")?)
        .next()
        .and_then(|node| node.value().attr("href"))
        .map(|url| base.join(url).map(|u| u.to_string()))
        .transpose()?;
    let mut note = String::new();
    for child in dd.children() {
        if let Some(text) = child.value().as_text() {
            note.push_str(text);
        } else if let Some(element) = ElementRef::wrap(child) &&
            element.value().classes().any(|class| class == "former")
        {
            note.push_str(&native_html::title(element));
        }
    }
    let note = note.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(Person { name, affiliation, url, note, orcid })
}

fn selector(css: &str) -> Result<Selector> {
    Selector::parse(css).map_err(|error| {
        anyhow::anyhow!("invalid collector selector {css}: {error}")
    })
}
fn text(element: ElementRef<'_>, css: &str) -> Result<String> {
    element
        .select(&selector(css)?)
        .next()
        .map(native_html::title)
        .ok_or_else(|| anyhow::anyhow!("missing published metadata {css}"))
}
