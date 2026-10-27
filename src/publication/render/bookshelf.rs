//! Publication of recorded BookPlans, with immutable prepared compiler jobs.
use {
    super::{
        book_worker,
        content::Entries,
        markup::{Markdown, html_escape},
        model::Plan,
        specification::{
            metadata,
            pages::{self, BookContext, Catalog},
        },
        typst,
    },
    crate::publication::{
        diagnostics::{Diagnostic, FailureMode, Severity},
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    serde::{Deserialize, Serialize},
    sha2::{Digest, Sha256},
    std::{
        collections::{HashMap, HashSet},
        path::Path,
    },
    tokio::{
        fs::File,
        io::{AsyncWriteExt, BufWriter},
        task::JoinSet,
    },
};
#[derive(Serialize, Deserialize)]
struct Book {
    id:           String,
    plan:         String,
    title:        String,
    html:         String,
    source:       String,
    pdf:          Option<String>,
    status:       String,
    reused:       bool,
    digest:       String,
    pdf_digest:   Option<String>,
    module:       bool,
    #[serde(skip)]
    source_bytes: usize,
    #[serde(skip)]
    root:         usize,
    #[serde(skip)]
    members:      HashSet<usize>,
}
#[derive(Serialize, Deserialize)]
struct Inventory {
    version:  u32,
    revision: String,
    books:    Vec<Book>,
}
async fn books(plan: &Plan, catalog: &Catalog) -> Result<Vec<Book>> {
    let view =
        RevisionView::new(plan.dataset.clone(), plan.handle.snapshot.clone());
    let mut rows = view.objects(&plan.handle.id, "bookPlan").await?;
    let mut books = Vec::new();
    let mut owned = HashSet::new();
    while let Some(row) = rows.next().await? {
        ensure!(
            books.len() < crate::publication::limits::SPECIFICATION_PAGES,
            "bookshelf exceeds book budget"
        );
        let oxrdf::Term::NamedNode(book) = &row["value"] else {
            anyhow::bail!("book plan must be an IRI")
        };
        ensure!(
            view.has_type(book, "BookPlan").await?,
            "missing book plan declaration"
        );
        let root = view.node(book, "bookRoot").await?;
        let root = *catalog
            .placements
            .get(root.as_str())
            .ok_or_else(|| anyhow::anyhow!("book root not selected"))?;
        let mut rows = view.objects(book, "bookPlacement").await?;
        let mut members = HashSet::new();
        while let Some(row) = rows.next().await? {
            let oxrdf::Term::NamedNode(placement) = &row["value"] else {
                anyhow::bail!("book placement must be an IRI")
            };
            let page = *catalog.placements.get(placement.as_str()).ok_or_else(
                || anyhow::anyhow!("book placement not selected"),
            )?;
            ensure!(
                page != 0 && owned.insert(page),
                "book placements overlap or include family front matter"
            );
            members.insert(page);
        }
        ensure!(members.contains(&root), "book omits its root");
        let subject = view.node(book, "bookSubject").await?;
        let route = crate::publication::dataset::identity("book-route", &[
            plan.target.as_str(),
            subject.as_str(),
        ]);
        let hash = route.as_str().rsplit(':').next().unwrap();
        books.push(Book {
            id: subject.as_str().into(),
            plan: book.as_str().into(),
            title: catalog.pages[root].title.clone(),
            html: catalog.pages[root].route.clone(),
            source: format!("book-{hash}.typ"),
            pdf: None,
            status: "pending".into(),
            reused: false,
            digest: String::new(),
            pdf_digest: None,
            module: catalog.pages[root].kind == "module",
            root,
            source_bytes: 0,
            members,
        });
    }
    ensure!(
        !books.is_empty(),
        "resolved bookshelf has no book plans"
    );
    books.sort_by(|a, b| (&a.title, &a.id).cmp(&(&b.title, &b.id)));
    Ok(books)
}
pub(super) fn compiler_digest() -> String {
    let mut hash = Sha256::new();
    hash.update(include_str!("engine.rs"));
    hash.update(include_str!("../../../Cargo.lock"));
    for bytes in [
        include_bytes!("fonts/LiberationSans-Regular.ttf").as_slice(),
        include_bytes!("fonts/LiberationSans-Bold.ttf").as_slice(),
        include_bytes!("fonts/LiberationSans-Italic.ttf").as_slice(),
        include_bytes!("fonts/LiberationSans-BoldItalic.ttf").as_slice(),
    ] {
        hash.update(bytes);
    }
    hex::encode(hash.finalize())
}
async fn file_hash(path: &Path, limit: u64) -> Result<String> {
    use tokio::io::AsyncReadExt;
    let meta = tokio::fs::symlink_metadata(path).await?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= limit,
        "invalid or oversized book artifact"
    );
    let mut file = File::open(path).await?;
    // Fixed heap allocation: never embed a 64 KiB buffer in every parent async
    // future.
    let mut buf = vec![0u8; 65536];
    let mut hash = Sha256::new();
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}
async fn old_inventory(directory: &Path) -> Result<HashMap<String, Book>> {
    let manifest = directory.join("manifest.yaml");
    if !tokio::fs::try_exists(&manifest).await? {
        return Ok(HashMap::new());
    }
    let manifest: super::model::PublicationManifest =
        serde_yaml::from_slice(&tokio::fs::read(manifest).await?)?;
    if !manifest.files.iter().any(|f| f == "books.json") {
        return Ok(HashMap::new());
    }
    let path = directory.join("books.json");
    ensure!(
        tokio::fs::metadata(&path).await?.len() <=
            crate::publication::limits::REFERENCE_INDEX_BYTES as u64,
        "book inventory exceeds byte budget"
    );
    let inventory: Inventory =
        serde_json::from_slice(&tokio::fs::read(path).await?)?;
    ensure!(
        inventory.version == 1 &&
            inventory.books.len() <=
                crate::publication::limits::SPECIFICATION_PAGES,
        "invalid book inventory"
    );
    let mut result = HashMap::new();
    for b in inventory.books {
        ensure!(
            result.insert(b.source.clone(), b).is_none(),
            "duplicate cached book identity"
        );
    }
    Ok(result)
}

pub(super) async fn write(
    plan: &Plan,
    stage: &Path,
    directory: &Path,
    mode: FailureMode,
    executable: Option<&Path>,
    jobs: usize,
) -> Result<(Vec<String>, Vec<Diagnostic>)> {
    let catalog = pages::catalog(plan).await?;
    let mut books = books(plan, &catalog).await?;
    let jobs = match plan.source.optional(&plan.target, "pdfWorkers").await? {
        None => jobs,
        Some(oxrdf::Term::Literal(v))
            if v.datatype().as_str() ==
                "http://www.w3.org/2001/XMLSchema#integer" =>
        {
            v.value().parse::<usize>()?
        },
        _ => anyhow::bail!("pdfWorkers must be an xsd:integer"),
    };
    ensure!(
        (1..=8).contains(&jobs),
        "pdfWorkers must be between one and eight"
    );
    let executable = book_worker::executable(executable)?;
    let mut ownership = HashMap::new();
    for (i, b) in books.iter().enumerate() {
        for p in &b.members {
            ensure!(
                ownership.insert(*p, i).is_none(),
                "overlapping book plans"
            );
        }
    }
    let mut files: Vec<String> = Vec::new();
    for b in &books {
        let mut file =
            BufWriter::new(File::create(stage.join(&b.source)).await?);
        file.write_all(
            format!(
                "// Book plan: {}\n// Book subject: {}\n",
                b.plan, b.id
            )
            .as_bytes(),
        )
        .await?;
        typst::begin_book(&mut file, plan, &b.title, &[]).await?;
        file.flush().await?;
        files.push(b.source.clone());
    }
    let mut active: Option<(usize, BufWriter<File>)> = None;
    let mut opened = vec![false; books.len()];
    let mut depths = vec![0; books.len()];
    let mut entries = Entries::new(plan).await?;
    let base = match plan.source.optional(&plan.target, "thisVersion").await? {
        Some(oxrdf::Term::NamedNode(url)) => url::Url::parse(url.as_str())?,
        _ => anyhow::bail!("bookshelf requires thisVersion URL"),
    };
    while let Some(mut entry) = entries.next(plan).await? {
        let page = catalog.placements[entry.placement.as_str()];
        let Some(&i) = ownership.get(&page) else {
            continue
        };
        if active.as_ref().map(|(book, _)| *book) != Some(i) {
            if let Some((_, mut file)) = active.take() {
                file.flush().await?;
            }
            active = Some((
                i,
                BufWriter::new(
                    tokio::fs::OpenOptions::new()
                        .append(true)
                        .open(stage.join(&books[i].source))
                        .await?,
                ),
            ));
        }
        let file = &mut active.as_mut().unwrap().1;
        let b = &books[i];
        if page == b.root && entry.first {
            depths[i] = entry.depth;
        }
        ensure!(
            depths[i] > 0,
            "book root must precede its chapters"
        );
        let root_module = catalog.pages[b.root].kind == "module";
        let offset = if root_module && page != b.root {
            depths[i]
        } else {
            depths[i] - 1
        };
        entry.depth = entry.depth.checked_sub(offset).ok_or_else(|| {
            anyhow::anyhow!("book chapter lies outside its root")
        })?;
        ensure!(entry.depth > 0, "invalid book chapter depth");
        if entry.first {
            if opened[i] {
                typst::close(file).await?;
            }
            opened[i] = true;
            if root_module && page == b.root {
                entry.kind = "abstract".into();
            } else {
                entry.kind = "section".into();
            }
        }
        let anchor = plan.anchor(&entry.placement, Some(&entry.fragment));
        let mut markdown = if entry.format == "text/markdown" {
            Some(
                Markdown::parse(
                    &entry.body,
                    plan,
                    &anchor,
                    &entry.fragment,
                    entry.depth,
                )
                .await?,
            )
        } else {
            None
        };
        if let Some(markdown) = &mut markdown {
            for event in &mut markdown.events {
                if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link {
                    dest_url,
                    ..
                }) = event &&
                    let Some(id) = dest_url.strip_prefix('#')
                {
                    let page = *catalog.owners.get(id).ok_or_else(|| {
                        anyhow::anyhow!("unresolved book link {id}")
                    })?;
                    if !b.members.contains(&page) {
                        *dest_url = base
                            .join(&format!(
                                "{}#{id}",
                                catalog.pages[page].route
                            ))?
                            .to_string()
                            .into();
                    }
                }
            }
        }
        typst::entry(
            file,
            plan,
            &entry,
            &anchor,
            markdown.as_ref(),
            0,
            &[],
        )
        .await?;
    }
    if let Some((_, mut file)) = active.take() {
        file.flush().await?;
    }
    for (i, b) in books.iter().enumerate() {
        let mut file = BufWriter::new(
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(stage.join(&b.source))
                .await?,
        );
        typst::end(&mut file, opened[i]).await?;
        file.flush().await?;
    }
    let previous = old_inventory(directory).await?;
    let compiler = compiler_digest();
    let mut pending = Vec::new();
    // Shared assets are immutable, verified KG assets. Include every retained
    // asset in the cache key; this safely invalidates books on asset changes.
    let mut asset_parts = Vec::new();
    let mut assets = plan.assets().await?;
    while let Some(row) = assets.next().await? {
        ensure!(
            asset_parts.len() < 256,
            "book asset budget exceeded"
        );
        let oxrdf::Term::NamedNode(asset) = &row["asset"] else {
            anyhow::bail!("asset must be named")
        };
        let route = plan.source.text(asset, "route").await?;
        asset_parts.push((
            asset.as_str().to_owned(),
            route.clone(),
            file_hash(
                &stage.join(route),
                crate::publication::image_assets::MAX_IMAGE_BYTES as u64,
            )
            .await?,
        ));
    }
    asset_parts.sort();
    let mut asset_hash = Sha256::new();
    for (id, route, hash) in asset_parts {
        asset_hash.update(id);
        asset_hash.update(route);
        asset_hash.update(hash);
    }
    let assets = hex::encode(asset_hash.finalize());
    for (i, b) in books.iter_mut().enumerate() {
        let source = stage.join(&b.source);
        ensure!(
            tokio::fs::metadata(&source).await?.len() <=
                crate::publication::limits::TYPST_SOURCE_BYTES,
            "book source exceeds compilation budget"
        );
        let mut hash = Sha256::new();
        hash.update(&compiler);
        hash.update(&assets);
        let text = tokio::fs::read_to_string(&source).await?;
        b.source_bytes = text.len();
        for line in text.lines() {
            if !line.starts_with("// Revision:") &&
                !line.starts_with("// Book plan:")
            {
                hash.update(line);
                hash.update(b"\n");
            }
        }
        b.digest = hex::encode(hash.finalize());
        if let Some(old) = previous
            .get(&b.source)
            .filter(|old| old.digest == b.digest && old.status == "ready")
        {
            let pdf = old.pdf.as_ref().ok_or_else(|| {
                anyhow::anyhow!("ready cached book has no PDF")
            })?;
            ensure!(
                pdf == &b.source.replace(".typ", ".pdf"),
                "invalid cached PDF route"
            );
            let digest = file_hash(
                &directory.join(pdf),
                crate::publication::limits::ARTIFACT_BYTES,
            )
            .await?;
            ensure!(
                old.pdf_digest.as_ref() == Some(&digest),
                "cached book PDF checksum mismatch for {}",
                b.id
            );
            tokio::fs::copy(directory.join(pdf), stage.join(pdf)).await?;
            b.pdf = Some(pdf.clone());
            b.pdf_digest = Some(digest);
            b.status = "ready".into();
            b.reused = true;
        } else {
            pending.push(i);
        }
    }
    pending.sort_by(|a, b| {
        books[*b]
            .source_bytes
            .cmp(&books[*a].source_bytes)
            .then(a.cmp(b))
    });
    tracing::info!(
        books = books.len(),
        workers = jobs,
        reused = books.len() - pending.len(),
        pending = pending.len(),
        "book PDF jobs prepared"
    );
    // Pin one executable image for this generation. Package upgrades or a
    // development rebuild must not change compilers halfway through a book set.
    let pinned_worker = if !pending.is_empty() {
        let worker = stage.join(format!(
            ".grapharch-book-worker{}",
            std::env::consts::EXE_SUFFIX
        ));
        let meta = tokio::fs::metadata(&executable).await?;
        ensure!(
            meta.is_file() && meta.len() <= 512 * 1024 * 1024,
            "worker executable exceeds byte budget"
        );
        tokio::fs::copy(&executable, &worker).await?;
        Some(worker)
    } else {
        None
    };
    let executable = pinned_worker.as_ref().unwrap_or(&executable);
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let mut running = JoinSet::new();
    let mut next = 0;
    let mut diagnostics = Vec::new();
    while next < pending.len() || !running.is_empty() {
        while next < pending.len() && running.len() < jobs {
            let i = pending[next];
            next += 1;
            let executable = executable.to_owned();
            let expected_engine = compiler.clone();
            let root = stage.to_owned();
            let source = books[i].source.clone();
            let cancel = cancel_rx.clone();
            running.spawn(async move {
                (
                    i,
                    book_worker::run(
                        &executable,
                        &root,
                        &source,
                        &expected_engine,
                        cancel,
                    )
                    .await,
                )
            });
        }
        let (i, result) = running.join_next().await.ok_or_else(|| {
            anyhow::anyhow!("book worker queue unexpectedly empty")
        })??;
        match result {
            Ok(()) => {
                let pdf = books[i].source.replace(".typ", ".pdf");
                books[i].pdf_digest = Some(
                    file_hash(
                        &stage.join(&pdf),
                        crate::publication::limits::ARTIFACT_BYTES,
                    )
                    .await?,
                );
                books[i].pdf = Some(pdf);
                books[i].status = "ready".into();
                tracing::info!(book=%books[i].title,subject=%books[i].id,"book PDF completed");
            },
            Err(error) => {
                if mode == FailureMode::FailFast {
                    cancel_tx.send_replace(true);
                    while running.join_next().await.is_some() {}
                    return Err(error.context(format!(
                        "PDF generation failed for book {} ({})",
                        books[i].title, books[i].id
                    )));
                }
                ensure!(
                    diagnostics.len() < 1024,
                    "book diagnostic budget exceeded"
                );
                let partial =
                    stage.join(&books[i].source).with_extension("pdf");
                match tokio::fs::symlink_metadata(&partial).await {
                    Ok(meta) => {
                        ensure!(
                            meta.is_file() && !meta.file_type().is_symlink(),
                            "worker left an invalid partial PDF"
                        );
                        tokio::fs::remove_file(partial).await?;
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                    Err(e) => return Err(e.into()),
                }
                books[i].status = "failed".into();
                diagnostics.push(Diagnostic {
                    severity:        Severity::Error,
                    code:            "book-render-failed".into(),
                    message:         error.to_string(),
                    reference:       books[i].id.clone(),
                    source_fragment: None,
                });
            },
        }
    }
    if let Some(worker) = pinned_worker {
        tokio::fs::remove_file(worker).await?;
    }
    let mut total_pdf_bytes = 0u64;
    for book in &books {
        if let Some(pdf) = &book.pdf {
            total_pdf_bytes = total_pdf_bytes
                .checked_add(tokio::fs::metadata(stage.join(pdf)).await?.len())
                .ok_or_else(|| {
                    anyhow::anyhow!("book PDF byte count overflow")
                })?;
        }
    }
    ensure!(
        total_pdf_bytes <= crate::publication::limits::RETAINED_ASSET_BYTES,
        "bookshelf PDFs exceed total byte budget"
    );
    let mut contexts = HashMap::new();
    let mut search_by_page: HashMap<&str, Vec<&pages::SearchEntry>> =
        HashMap::new();
    for entry in &catalog.search {
        let route = entry
            .href
            .split_once('#')
            .ok_or_else(|| anyhow::anyhow!("search entry lacks anchor"))?
            .0;
        search_by_page.entry(route).or_default().push(entry);
    }

    for b in &books {
        let nav = b.source.replace(".typ", "-navigation.json");
        let search = b.source.replace(".typ", "-search.json");
        let mut members = b.members.iter().copied().collect::<Vec<_>>();
        members.sort();
        let local: HashMap<usize, usize> =
            members.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let navigation:Vec<_>=members.iter().map(|p|serde_json::json!({"title":catalog.pages[*p].title,"href":catalog.pages[*p].route,"parent":catalog.pages[*p].parent.and_then(|p|local.get(&p).copied())})).collect();
        tokio::fs::write(stage.join(&nav), serde_json::to_vec(&navigation)?)
            .await?;
        let search_entries: Vec<_> = members
            .iter()
            .flat_map(|p| {
                search_by_page
                    .get(catalog.pages[*p].route.as_str())
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect();
        tokio::fs::write(
            stage.join(&search),
            serde_json::to_vec(&search_entries)?,
        )
        .await?;
        files.extend([nav.clone(), search.clone()]);
        if let Some(pdf) = &b.pdf {
            files.push(pdf.clone());
        }
        for p in &b.members {
            contexts.insert(*p, BookContext {
                title:      b.title.clone(),
                route:      b.html.clone(),
                pdf:        b.pdf.clone(),
                navigation: nav.clone(),
                search:     search.clone(),
            });
        }
    }
    let (page_files, mut page_diagnostics) =
        pages::write_catalog(plan, stage, mode, &catalog, Some(&contexts))
            .await?;
    files.extend(page_files);
    diagnostics.append(&mut page_diagnostics);
    catalogue(plan, stage, &catalog, &books, &diagnostics).await?;
    files.push("index.html".into());
    let inventory = Inventory {
        version: 1,
        revision: plan.revision.as_str().into(),
        books,
    };
    let bytes = serde_json::to_vec(&inventory)?;
    ensure!(
        bytes.len() <= crate::publication::limits::REFERENCE_INDEX_BYTES,
        "books inventory exceeds byte budget"
    );
    tokio::fs::write(stage.join("books.json"), bytes).await?;
    files.push("books.json".into());
    files.sort();
    files.dedup();
    Ok((files, diagnostics))
}
async fn catalogue(
    plan: &Plan,
    stage: &Path,
    catalog: &Catalog,
    books: &[Book],
    diagnostics: &[Diagnostic],
) -> Result<()> {
    let header = metadata::header(plan).await?;
    let accent = header.color.clone();
    let header = pages::page_header(&header.html, &plan.title, None)?;
    let mut html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta \
         name=\"viewport\" \
         content=\"width=device-width,initial-scale=1\"><meta \
         name=\"grapharch-revision\" content=\"{}\"><title>{}</title><link \
         rel=\"stylesheet\" href=\"style.css\"></head><body \
         class=\"bookshelf\" \
         style=\"--authority-color:{accent}\"><main>{header}<nav id=\"toc\" \
         class=\"catalogue-tools\" \
         data-search-index=\"search-index.json\"><div \
         class=\"toc-tools\"><button type=\"button\" data-search-open \
         aria-haspopup=\"dialog\" aria-controls=\"spec-search\">Search this \
         bookshelf <kbd>Ctrl K</kbd></button></div></nav><h2>Module \
         books</h2><p>Choose a book to read its specification or download its \
         PDF.</p>",
        html_escape(plan.revision.as_str()),
        html_escape(&plan.title)
    );
    let mut shelves: std::collections::BTreeMap<Option<usize>, Vec<&Book>> =
        std::collections::BTreeMap::new();
    for b in books.iter().filter(|b| b.module) {
        shelves
            .entry(catalog.pages[b.root].parent)
            .or_default()
            .push(b);
    }
    for (parent, books) in shelves {
        if let Some(parent) = parent {
            let series = &catalog.pages[parent];
            html.push_str(&format!(
                "<section class=\"book-shelf\"><h3><a href=\"{}\">{}</a></h3>",
                html_escape(&series.route),
                html_escape(&series.title)
            ));
        } else {
            html.push_str(
                "<section class=\"book-shelf\"><h3>Independent module \
                 books</h3>",
            );
        }
        html.push_str("<div class=\"book-grid\">");
        for b in books {
            card(&mut html, b);
        }
        html.push_str("</div></section>");
    }
    html.push_str(
        "<details class=\"additional-books\"><summary>Additional \
         ontologies</summary><p>These ontologies have no selected module \
         book. They remain available as individual books.</p><div \
         class=\"book-grid\">",
    );
    for b in books.iter().filter(|b| !b.module) {
        card(&mut html, b);
    }
    html.push_str(
        "</div></details><section><h2>Book series</h2><div \
         class=\"series-list\">",
    );
    for (i, p) in catalog
        .pages
        .iter()
        .enumerate()
        .filter(|(_, p)| p.kind == "module")
    {
        if !books.iter().any(|b| b.root == i) {
            html.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                html_escape(&p.route),
                html_escape(&p.title)
            ));
        }
    }
    html.push_str(
        "</div></section><footer>Generated by Grapharch from its \
         documentation graph.</footer></main></body></html>",
    );
    let mut publication_details = String::new();
    for predicate in [
        "http://purl.org/dc/terms/description",
        "http://www.w3.org/2000/01/rdf-schema#comment",
    ] {
        if let Some(value) = plan
            .source
            .optional_by(
                &plan.target,
                &oxrdf::NamedNode::new_unchecked(predicate),
            )
            .await?
        {
            let oxrdf::Term::Literal(value) = value else {
                anyhow::bail!("publication description must be literal")
            };
            publication_details
                .push_str(&format!("<p>{}</p>", html_escape(value.value())));
        }
    }
    publication_details.push_str("<nav aria-label=\"Publication details\">");
    for (i, p) in catalog
        .pages
        .iter()
        .enumerate()
        .filter(|(_, p)| p.kind == "section")
    {
        if i != 0 {
            publication_details.push_str(&format!(
                "<p><a href=\"{}\">{}</a></p>",
                html_escape(&p.route),
                html_escape(&p.title)
            ));
        }
    }
    publication_details.push_str("</nav>");
    let mut details = format!(
        "<details><summary>About this \
         publication</summary>{publication_details}</details>"
    );
    if !diagnostics.is_empty() {
        details.push_str(
            "<section id=\"publication-diagnostics\"><h2>Publication \
             diagnostics</h2><ol>",
        );
        for error in diagnostics {
            details.push_str(&format!(
                "<li><strong>{}</strong>: \
                 {}<details><summary>Details</summary><pre>{}</pre></\
                 details></li>",
                html_escape(&error.reference),
                html_escape(&error.code),
                html_escape(&error.message)
            ));
        }
        details.push_str("</ol></section>");
    }
    html = html.replace("<footer>", &format!("{details}<footer>"));
    let controls = include_str!("specification/search.html").replace(
        "Search this specification",
        "Search this bookshelf",
    );
    html = html.replace(
        "</body>",
        &format!(
            "{controls}<script>{}</script></body>",
            include_str!("specification/sidebar.js")
        ),
    );
    tokio::fs::write(stage.join("index.html"), html).await?;
    Ok(())
}
fn card(html: &mut String, b: &Book) {
    html.push_str(&format!(
        "<article class=\"book-card\"><a class=\"book-cover\" \
         href=\"{}\"><span>Ontology specification</span><h3>{}</h3></a><div \
         class=\"book-actions\"><a href=\"{}\">Open book</a>",
        html_escape(&b.html),
        html_escape(&b.title),
        html_escape(&b.html)
    ));
    if let Some(pdf) = &b.pdf {
        html.push_str(&format!(
            "<a href=\"{}\" type=\"application/pdf\">↓ PDF</a>",
            html_escape(pdf)
        ));
    } else {
        html.push_str(
            "<strong class=\"book-error\">PDF generation failed</strong>",
        );
    }
    html.push_str("</div></article>");
}

pub(super) async fn manifest_books(
    plan: &Plan,
    stage: &Path,
) -> Result<Vec<super::BookPublication>> {
    let bytes = tokio::fs::read(stage.join("books.json")).await?;
    ensure!(
        bytes.len() <= crate::publication::limits::REFERENCE_INDEX_BYTES,
        "book inventory exceeds byte budget"
    );
    let inventory: Inventory = serde_json::from_slice(&bytes)?;
    ensure!(
        inventory.version == 1 && inventory.revision == plan.revision.as_str(),
        "book inventory does not match pinned revision"
    );
    Ok(inventory
        .books
        .into_iter()
        .map(|b| {
            super::BookPublication {
                plan:       b.plan,
                subject:    b.id,
                html:       b.html,
                pdf:        b.pdf,
                status:     b.status,
                reused:     b.reused,
                digest:     b.digest,
                pdf_digest: b.pdf_digest,
            }
        })
        .collect())
}
