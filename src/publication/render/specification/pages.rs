//! Component-sized specifications, with one bounded routing/search index shared
//! by every page. All content and hierarchy come from the pinned plan.
use {
    super::{
        super::{
            content::{Entries, Entry, code_language},
            markup::{Markdown, html_escape},
            model::Plan,
        },
        metadata,
        native,
        outline::{Outline, Toc, markdown_html, permalink},
    },
    crate::publication::{
        diagnostics::{Diagnostic, FailureMode, Severity},
        native_html,
    },
    anyhow::{Result, ensure},
    serde::Serialize,
    std::{collections::HashMap, path::Path},
    tokio::{
        fs::File,
        io::{AsyncWriteExt, BufWriter},
    },
};

#[derive(Serialize)]
pub(in crate::publication::render) struct SearchEntry {
    pub(in crate::publication::render) title:      String,
    pub(in crate::publication::render) href:       String,
    pub(in crate::publication::render) kind:       String,
    pub(in crate::publication::render) context:    String,
    pub(in crate::publication::render) identifier: String,
}
pub(in crate::publication::render) struct Page {
    pub(in crate::publication::render) title:  String,
    pub(in crate::publication::render) route:  String,
    pub(in crate::publication::render) kind:   String,
    pub(in crate::publication::render) parent: Option<usize>,
    toc:                                       Toc,
    contents:                                  String,
}
pub(in crate::publication::render) struct Catalog {
    pub(in crate::publication::render) pages:      Vec<Page>,
    pub(in crate::publication::render) owners:     HashMap<String, usize>,
    pub(in crate::publication::render) placements: HashMap<String, usize>,
    pub(in crate::publication::render) search:     Vec<SearchEntry>,
}

impl Catalog {
    fn anchor(&mut self, id: String, page: usize) -> Result<()> {
        ensure!(
            ![
                "document",
                "toc",
                "toc-title",
                "toc-resizer",
                "spec-search",
                "spec-search-title",
                "spec-search-query",
                "spec-search-kind",
                "spec-search-status",
                "spec-search-results"
            ]
            .contains(&id.as_str()),
            "publication anchor conflicts with navigation: {id}"
        );
        ensure!(
            self.owners.len() <
                crate::publication::limits::SPECIFICATION_ANCHORS,
            "specification anchor budget exceeded"
        );
        ensure!(
            self.owners.insert(id.clone(), page).is_none(),
            "duplicate publication anchor {id}"
        );
        Ok(())
    }

    fn href(&self, id: &str, current: usize) -> Result<String> {
        let page = *self.owners.get(id).ok_or_else(|| {
            anyhow::anyhow!("unresolved publication reference #{id}")
        })?;
        Ok(if page == current {
            format!("#{id}")
        } else {
            format!("{}#{id}", self.pages[page].route)
        })
    }
}
fn local_outline(home: bool) -> Outline {
    if home {
        Outline::default()
    } else {
        Outline::page()
    }
}
async fn body(
    entry: &Entry,
    plan: &Plan,
    outline: &mut Outline,
) -> Result<String> {
    let anchor = plan.anchor(&entry.placement, Some(&entry.fragment));
    if entry.format == "text/html" {
        Ok(
            native_html::read(&entry.body, None, &mut |depth| {
                outline.heading_number(depth)
            })?
            .html,
        )
    } else if entry.format == "text/markdown" {
        let markdown =
            Markdown::parse(&entry.body, plan, &anchor, &entry.fragment, 1)
                .await?;
        markdown_html(&markdown, outline)
    } else {
        Ok(format!(
            "<pre><code class=\"language-{}\">{}</code></pre>",
            code_language(&entry.format)?,
            html_escape(&entry.body)
        ))
    }
}
pub(in crate::publication::render) async fn catalog(
    plan: &Plan,
) -> Result<Catalog> {
    let mut catalog = Catalog {
        pages:      vec![Page {
            title:    plan.title.clone(),
            route:    "index.html".into(),
            kind:     "section".into(),
            parent:   None,
            toc:      Toc::new(),
            contents: String::new(),
        }],
        owners:     HashMap::new(),
        placements: HashMap::new(),
        search:     Vec::new(),
    };
    let toc_depth = plan.toc_depth().await?.unwrap_or(usize::MAX);
    let mut entries = Entries::new(plan).await?;
    let mut page = 0;
    let mut first_section = false;
    let mut outline = Outline::default();
    while let Some(entry) = entries.next(plan).await? {
        if entry.first {
            if !matches!(entry.kind.as_str(), "abstract" | "status") {
                if first_section {
                    ensure!(
                        catalog.pages.len() <
                            crate::publication::limits::SPECIFICATION_PAGES,
                        "specification page budget exceeded"
                    );
                    page = catalog.pages.len();
                    let kind = kind(&entry, plan).await?;
                    catalog.pages.push(Page {
                        title:    entry.title.clone(),
                        route:    format!(
                            "section-{}.html",
                            crate::publication::dataset::identity("page", &[
                                entry.placement.as_str()
                            ])
                            .as_str()
                            .rsplit(':')
                            .next()
                            .unwrap()
                        ),
                        kind:     kind.into(),
                        parent:   match plan
                            .source
                            .optional(&entry.placement, "parentPlacement")
                            .await?
                        {
                            Some(oxrdf::Term::NamedNode(parent)) => {
                                Some(
                                    *catalog
                                        .placements
                                        .get(parent.as_str())
                                        .ok_or_else(|| {
                                            anyhow::anyhow!(
                                                "page parent must precede \
                                                 child"
                                            )
                                        })?,
                                )
                            },
                            None => None,
                            _ => anyhow::bail!("page parent must be an IRI"),
                        },
                        toc:      Toc::new(),
                        contents: String::new(),
                    });
                    outline = local_outline(false);
                } else {
                    first_section = true;
                }
            }
            catalog
                .placements
                .insert(entry.placement.as_str().into(), page);
            let id = plan.anchor(&entry.placement, None);
            catalog.anchor(id.clone(), page)?;
            let label = outline.component(&entry.kind, &entry.title, 1)?;
            let p = &mut catalog.pages[page];
            p.contents.push_str(&p.toc.entry(1, &id, &label)?);
            catalog.search.push(SearchEntry {
                title:      entry.title.clone(),
                href:       format!("{}#{id}", catalog.pages[page].route),
                kind:       catalog.pages[page].kind.clone(),
                context:    plan.title.clone(),
                identifier: subjects(&entry, plan)
                    .await?
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            });
        }
        let anchor = plan.anchor(&entry.placement, Some(&entry.fragment));
        catalog.anchor(anchor, page)?;
        for id in native::aliases(&entry, plan).await? {
            catalog.anchor(id, page)?;
        }
        // Parse only one capped fragment at a time; retain titles/routes, never
        // bodies.
        let rendered = body(&entry, plan, &mut outline).await?;
        let doc = scraper::Html::parse_fragment(&rendered);
        let selector = scraper::Selector::parse("[id]").unwrap();
        for element in doc.select(&selector) {
            catalog.anchor(element.value().attr("id").unwrap().into(), page)?;
        }
        let headings = scraper::Selector::parse("h1,h2,h3,h4,h5,h6").unwrap();
        for h in doc.select(&headings) {
            let Some(id) = h.value().attr("id") else {
                continue
            };
            let depth = h
                .value()
                .attr("aria-level")
                .unwrap_or(&h.value().name()[1..])
                .parse::<usize>()? -
                1;
            let title = h
                .text()
                .collect::<String>()
                .trim_end_matches('§')
                .trim()
                .to_owned();
            let p = &mut catalog.pages[page];
            if depth <= toc_depth {
                p.contents.push_str(&p.toc.entry(depth, id, &title)?);
            }
            catalog.search.push(SearchEntry {
                title,
                href: format!("{}#{id}", p.route),
                kind: "section".into(),
                context: p.title.clone(),
                identifier: String::new(),
            });
        }
    }
    ensure!(
        first_section,
        "paginated specification requires at least one section"
    );
    for p in &mut catalog.pages {
        p.contents.push_str(&p.toc.finish());
    }
    Ok(catalog)
}

pub(in crate::publication::render) async fn write(
    plan: &Plan,
    stage: &Path,
    mode: FailureMode,
) -> Result<(Vec<String>, Vec<Diagnostic>)> {
    let catalog = catalog(plan).await?;
    write_catalog(plan, stage, mode, &catalog, None).await
}

pub(in crate::publication::render) struct BookContext {
    pub title:      String,
    pub route:      String,
    pub pdf:        Option<String>,
    pub navigation: String,
    pub search:     String,
}
pub(in crate::publication::render) async fn write_catalog(
    plan: &Plan,
    stage: &Path,
    mode: FailureMode,
    catalog: &Catalog,
    books: Option<&HashMap<usize, BookContext>>,
) -> Result<(Vec<String>, Vec<Diagnostic>)> {
    let header = metadata::header(plan).await?;
    let mut files = vec![
        "style.css".into(),
        "search-index.json".into(),
        "navigation.json".into(),
    ];
    let search = serde_json::to_vec(&catalog.search)?;
    ensure!(
        search.len() <= crate::publication::limits::REFERENCE_INDEX_BYTES,
        "specification search index byte budget exceeded"
    );
    tokio::fs::write(stage.join("search-index.json"), search).await?;
    let navigation: Vec<_> = catalog
        .pages
        .iter()
        .map(|p| {
            Navigation {
                title:  &p.title,
                href:   &p.route,
                parent: p.parent,
            }
        })
        .collect();
    tokio::fs::write(
        stage.join("navigation.json"),
        serde_json::to_vec(&navigation)?,
    )
    .await?;
    let family_html = "<details class=\"family-navigation\"><summary>Browse \
                       this family</summary><p><a href=\"index.html\">Family \
                       overview</a></p><div data-family-tree \
                       data-navigation-index=\"navigation.json\"></div></\
                       details>";
    let mut entries = Entries::new(plan).await?;
    let mut current = None;
    let mut section_open = false;
    let mut file: Option<BufWriter<File>> = None;
    let mut outline = Outline::default();
    let mut diagnostics = Vec::new();
    while let Some(entry) = entries.next(plan).await? {
        let page = catalog.placements[entry.placement.as_str()];
        if books.is_some() && page == 0 {
            continue;
        }
        if current != Some(page) {
            if let Some(mut previous) = file.take() {
                if section_open {
                    previous.write_all(b"</section>").await?;
                }
                end(&mut previous).await?;
            }
            let p = &catalog.pages[page];
            files.push(p.route.clone());
            let mut next =
                BufWriter::new(File::create(stage.join(&p.route)).await?);
            let context = books.and_then(|books| books.get(&page));
            let book_breadcrumb = context
                .filter(|b| b.route != p.route)
                .map(|b| {
                    format!(
                        "<a href=\"{}\">{}</a> / ",
                        html_escape(&b.route),
                        html_escape(&b.title)
                    )
                })
                .unwrap_or_default();
            let search_index = context
                .map(|b| b.search.as_str())
                .unwrap_or("search-index.json");
            let mut header_html = page_header(
                &header.html,
                &p.title,
                context.and_then(|b| b.pdf.as_deref()),
            )?;
            if context.is_some_and(|book| book.pdf.is_none()) {
                header_html.push_str(
                    "<aside class=\"markdown-alert markdown-alert-caution\" \
                     role=\"alert\"><p>PDF generation failed. The HTML book \
                     is available. See the <a \
                     href=\"index.html#publication-diagnostics\">publication \
                     diagnostics</a>.</p></aside>",
                );
            }
            let toc_title = if context.is_some() {
                "Book contents"
            } else {
                "On this page"
            };
            let contents = if let Some(context) = context {
                format!(
                    "<p class=\"book-contents-title\"><a \
                     href=\"{}\">{}</a></p><div class=\"book-navigation\" \
                     data-book-navigation data-navigation-index=\"{}\"><div \
                     data-local-contents>{}</div></div>",
                    html_escape(&context.route),
                    html_escape(&context.title),
                    html_escape(&context.navigation),
                    p.contents
                )
            } else {
                p.contents.clone()
            };
            let family_html: String = if books.is_some() {
                "<p><a href=\"index.html\">Back to bookshelf</a></p>".into()
            } else {
                family_html.into()
            };
            next.write_all(
                format!(
                    "<!doctype html><html lang=\"en\"><head><meta \
                     charset=\"utf-8\"><meta name=\"viewport\" \
                     content=\"width=device-width,initial-scale=1\"><meta \
                     name=\"grapharch-revision\" content=\"{}\"><title>{} · \
                     {}</title><link rel=\"stylesheet\" \
                     href=\"style.css\"><link rel=\"icon\" \
                     href=\"data:,\"></head><body class=\"specification \
                     specification-pages\" style=\"--authority-color:{}\"><a \
                     class=\"skip-link\" href=\"#document\">Skip to \
                     document</a><main id=\"document\"><p \
                     class=\"family-breadcrumb\"><a \
                     href=\"index.html\">{}</a> / \
                     {book_breadcrumb}{}</p>{header_html}<nav id=\"toc\" \
                     class=\"toc\" role=\"doc-toc\" \
                     aria-labelledby=\"toc-title\" \
                     data-search-index=\"{search_index}\"><h2 \
                     id=\"toc-title\">{toc_title}</h2>{}{}{family_html}</nav>",
                    html_escape(plan.revision.as_str()),
                    html_escape(&p.title),
                    html_escape(&plan.title),
                    header.color,
                    html_escape(&plan.title),
                    html_escape(&p.title),
                    include_str!("navigation.html"),
                    contents
                )
                .as_bytes(),
            )
            .await?;
            file = Some(next);
            section_open = false;
            current = Some(page);
            outline = local_outline(page == 0);
        }
        let file = file.as_mut().unwrap();
        if entry.first {
            if section_open {
                file.write_all(b"</section>").await?;
            }
            section_open = true;
            let id = plan.anchor(&entry.placement, None);
            let label = outline.component(&entry.kind, &entry.title, 1)?;
            file.write_all(
                format!(
                    "<section id=\"{id}\" data-page-body=\"{}\"><h2>{}{}</h2>",
                    html_escape(&entry.title),
                    html_escape(&label),
                    permalink(&id)
                )
                .as_bytes(),
            )
            .await?;
        }
        let anchor = plan.anchor(&entry.placement, Some(&entry.fragment));
        for id in native::aliases(&entry, plan).await? {
            file.write_all(
                format!("<span id=\"{}\"></span>", html_escape(&id)).as_bytes(),
            )
            .await?;
        }
        let rendered = body(&entry, plan, &mut outline).await?;
        let (routed, errors) = route_links(&rendered, catalog, page)?;
        for (reference, message) in errors {
            ensure!(mode == FailureMode::KeepGoing, "{message}");
            ensure!(
                diagnostics.len() < 1024,
                "diagnostic budget exceeded"
            );
            file.write_all(
                format!(
                    "<aside class=\"markdown-alert markdown-alert-caution\" \
                     role=\"alert\"><p \
                     class=\"markdown-alert-title\">Publication \
                     diagnostic</p><p>{}</p></aside>",
                    html_escape(&message)
                )
                .as_bytes(),
            )
            .await?;
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "unresolved-native-reference".into(),
                message,
                reference,
                source_fragment: Some(entry.fragment.as_str().into()),
            });
        }
        file.write_all(
            format!(
                "<div class=\"fragment\" id=\"{anchor}\" \
                 data-fragment=\"{}\">{routed}</div>",
                html_escape(entry.fragment.as_str())
            )
            .as_bytes(),
        )
        .await?;
    }
    if let Some(mut file) = file {
        if section_open {
            file.write_all(b"</section>").await?;
        }
        end(&mut file).await?;
    }
    Ok((files, diagnostics))
}
async fn end(file: &mut BufWriter<File>) -> Result<()> {
    file.write_all(b"<footer>Generated by Grapharch</footer></main>")
        .await?;
    file.write_all(include_str!("search.html").as_bytes())
        .await?;
    file.write_all(b"<div id=\"toc-resizer\" class=\"toc-resizer\" role=\"separator\" aria-label=\"Resize table of contents\" aria-controls=\"toc\" aria-orientation=\"vertical\" tabindex=\"0\" hidden></div><script>").await?;
    file.write_all(include_str!("sidebar.js").as_bytes())
        .await?;
    file.write_all(b"</script></body></html>").await?;
    file.flush().await?;
    Ok(())
}

pub(in crate::publication::render) fn page_header(
    header: &str,
    title: &str,
    pdf: Option<&str>,
) -> Result<String> {
    let mut document = scraper::Html::parse_fragment(header);
    let title_id = document
        .select(&scraper::Selector::parse("h1").unwrap())
        .next()
        .and_then(|h| h.first_child())
        .map(|n| n.id())
        .ok_or_else(|| anyhow::anyhow!("missing publication title"))?;
    let pdf_id = document
        .select(&scraper::Selector::parse("a.pdf-download").unwrap())
        .next()
        .map(|a| a.id())
        .ok_or_else(|| anyhow::anyhow!("missing publication PDF control"))?;
    let mut node = document.tree.get_mut(title_id).unwrap();
    let scraper::Node::Text(text) = node.value() else {
        anyhow::bail!("publication title must be text")
    };
    text.text = title.into();
    if let Some(pdf) = pdf {
        let mut node = document.tree.get_mut(pdf_id).unwrap();
        let scraper::Node::Element(element) = node.value() else {
            unreachable!()
        };
        for (name, value) in element.attrs.iter_mut() {
            if &*name.local == "href" {
                *value = pdf.into();
            }
        }
    } else {
        document.tree.get_mut(pdf_id).unwrap().detach();
    }
    Ok(document.root_element().inner_html())
}

pub(super) async fn kind(entry: &Entry, plan: &Plan) -> Result<&'static str> {
    let subjects = subjects(entry, plan).await?;
    // A component documenting several subjects is a general section, rather
    // than an individual ontology/module page.
    if subjects.len() != 1 {
        return Ok("section");
    }
    let subject = &subjects[0];
    let mut types = plan
        .source
        .objects_by(
            subject,
            &oxrdf::NamedNode::new_unchecked(crate::publication::RDF_TYPE),
        )
        .await?;
    let mut ontology = false;
    let mut module = false;
    let mut count = 0;
    while let Some(row) = types.next().await? {
        count += 1;
        ensure!(
            count <= 1024,
            "page subject exceeds type assertion budget"
        );
        ontology |= row["value"] ==
            oxrdf::Term::NamedNode(oxrdf::NamedNode::new_unchecked(
                "http://www.w3.org/2002/07/owl#Ontology",
            ));
        module |= row["value"] ==
            oxrdf::Term::NamedNode(crate::publication::term(
                "OntologyModule",
            ));
    }
    Ok(if ontology {
        "ontology"
    } else if module {
        "module"
    } else {
        "section"
    })
}

#[derive(Serialize)]
struct Navigation<'a> {
    title:  &'a str,
    href:   &'a str,
    parent: Option<usize>,
}

async fn subjects(entry: &Entry, plan: &Plan) -> Result<Vec<oxrdf::NamedNode>> {
    let mut rows = plan.source.objects(&entry.fragment, "subject").await?;
    let mut subjects = Vec::new();
    while let Some(row) = rows.next().await? {
        ensure!(
            subjects.len() < 1024,
            "fragment exceeds subject navigation budget"
        );
        let oxrdf::Term::NamedNode(subject) = &row["value"] else {
            anyhow::bail!("fragment subject must be an IRI")
        };
        subjects.push(subject.clone());
    }
    ensure!(
        !subjects.is_empty(),
        "fragment has no documentable subject"
    );
    subjects.sort();
    Ok(subjects)
}

fn route_links(
    html: &str,
    catalog: &Catalog,
    current: usize,
) -> Result<(String, Vec<(String, String)>)> {
    // Parse one capped fragment. Mutate only actual HTML link attributes;
    // quoted source examples and code text must never be interpreted as links.
    let mut document = scraper::Html::parse_fragment(html);
    let selector = scraper::Selector::parse("a[href]").unwrap();
    let nodes: Vec<_> = document.select(&selector).map(|a| a.id()).collect();
    let mut errors = Vec::new();
    for id in nodes {
        let mut node = document.tree.get_mut(id).unwrap();
        let scraper::Node::Element(element) = node.value() else {
            unreachable!()
        };
        for (name, value) in element.attrs.iter_mut() {
            if &*name.local != "href" {
                continue;
            }
            let Some(anchor) = value.strip_prefix('#') else {
                continue;
            };
            match catalog.href(anchor, current) {
                Ok(href) => *value = href.into(),
                Err(error) => {
                    ensure!(
                        errors.len() < 1024,
                        "fragment diagnostic budget exceeded"
                    );
                    errors.push((value.to_string(), error.to_string()));
                },
            }
        }
    }
    Ok((document.root_element().inner_html(), errors))
}
