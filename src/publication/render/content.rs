use {
    super::{
        html,
        markup::Markdown,
        model::{Plan, node, title},
        typst,
    },
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
    std::path::Path,
    tokio::{
        fs::File,
        io::{AsyncWriteExt, BufWriter},
    },
};

const MAX_ENTRIES: u64 = crate::publication::limits::PUBLICATION_ENTRIES;
const MAX_CONTENT_BYTES: u64 =
    crate::publication::limits::PUBLICATION_CONTENT_BYTES;

pub(super) struct Entry {
    pub fragment:  NamedNode,
    pub placement: NamedNode,
    pub title:     String,
    pub kind:      String,
    pub body:      String,
    pub format:    String,
    pub first:     bool,
    pub depth:     usize,
}

pub(super) struct Entries {
    rows:         crate::publication::dataset::Rows,
    previous:     Option<NamedNode>,
    count:        u64,
    bytes:        u64,
    depths:       std::collections::HashMap<NamedNode, usize>,
    pub appendix: u32,
}

impl Entries {
    pub async fn new(plan: &Plan) -> Result<Self> {
        Ok(Self {
            rows:     plan.entries().await?,
            previous: None,
            count:    0,
            bytes:    0,
            depths:   Default::default(),
            appendix: 0,
        })
    }

    pub async fn next(&mut self, plan: &Plan) -> Result<Option<Entry>> {
        let Some(row) = self.rows.next().await? else {
            return Ok(None)
        };
        self.count += 1;
        ensure!(
            self.count <= MAX_ENTRIES,
            "target exceeds the {}-entry rendering budget",
            MAX_ENTRIES
        );
        let fragment = node(&row["fragment"])?.clone();
        let placement = node(&row["placement"])?.clone();
        let component = node(&row["component"])?;
        let body = plan.source.text(&fragment, "body").await?;
        ensure!(
            body.len() <= 1024 * 1024,
            "fragment exceeds the 1 MiB rendering budget"
        );
        self.bytes += body.len() as u64;
        ensure!(
            self.bytes <= MAX_CONTENT_BYTES,
            "target exceeds the {}-byte content budget; select a smaller \
             publication scope",
            MAX_CONTENT_BYTES
        );
        let first = self.previous.as_ref() != Some(&placement);
        let kind = plan.section_kind(&placement).await?;
        let depth = if first {
            let depth = match plan
                .source
                .optional(&placement, "parentPlacement")
                .await?
            {
                None => 1,
                Some(oxrdf::Term::NamedNode(parent)) => {
                    self.depths.get(&parent).ok_or_else(|| {
                        anyhow::anyhow!(
                            "parent placement must precede its selected \
                             child: {parent}"
                        )
                    })? + 1
                },
                Some(_) => {
                    anyhow::bail!("parentPlacement must identify a placement")
                },
            };
            ensure!(
                depth <= 8,
                "publication component hierarchy exceeds eight levels"
            );
            self.depths.insert(placement.clone(), depth);
            depth
        } else {
            self.depths[&placement]
        };
        let entry = Entry {
            depth,
            format: plan.source.text(&fragment, "format").await?,
            title: title(&plan.source, component).await?,
            fragment,
            placement: placement.clone(),
            body,
            kind,
            first,
        };
        ensure!(
            entry.depth == 1 || entry.format != "text/html",
            "nested native HTML components require explicit heading \
             remapping; use Markdown for module references"
        );
        ensure!(
            entry.kind != "section" || self.appendix == 0,
            "numbered main sections cannot follow appendices"
        );
        if entry.first && entry.kind == "appendix" {
            self.appendix += 1;
        }
        self.previous = Some(placement);
        Ok(Some(entry))
    }
}

pub(super) async fn write(
    plan: &Plan,
    stage: &Path,
    mode: crate::publication::diagnostics::FailureMode,
) -> Result<Vec<crate::publication::diagnostics::Diagnostic>> {
    if plan.renderer == "specification" {
        return super::specification::write(plan, stage, mode).await;
    }
    let mut file = BufWriter::new(
        File::create(stage.join(
            if plan.is_html() {
                "index.html"
            } else {
                "main.typ"
            },
        ))
        .await?,
    );
    if plan.is_html() {
        html::begin(&mut file, plan).await?;
    } else {
        typst::begin(&mut file, plan, &[]).await?;
    }
    let mut entries = Entries::new(plan).await?;
    let mut previous = None;
    while let Some(entry) = entries.next(plan).await? {
        if entry.first && previous.is_some() {
            if plan.is_html() {
                html::close(&mut file).await?;
            } else {
                typst::close(&mut file).await?;
            }
        }
        let anchor = plan.anchor(&entry.placement, Some(&entry.fragment));
        let markdown = if entry.format == "text/markdown" {
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
        if plan.is_html() {
            html::entry(
                &mut file,
                plan,
                &entry,
                &anchor,
                markdown.as_ref(),
                entries.appendix,
            )
            .await?;
        } else {
            typst::entry(
                &mut file,
                plan,
                &entry,
                &anchor,
                markdown.as_ref(),
                entries.appendix,
                &[],
            )
            .await?;
        }
        previous = Some(entry.placement.clone());
    }
    if plan.is_html() {
        html::end(&mut file, previous.is_some()).await?;
    } else {
        typst::end(&mut file, previous.is_some()).await?;
    }
    file.flush().await?;
    Ok(Vec::new())
}

pub(super) async fn write_typst(
    plan: &Plan,
    stage: &Path,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<()> {
    let mut file = BufWriter::new(File::create(stage.join("main.typ")).await?);
    typst::begin(&mut file, plan, diagnostics).await?;
    let mut entries = Entries::new(plan).await?;
    let mut open = false;
    let mut main_started = false;
    while let Some(entry) = entries.next(plan).await? {
        if entry.first {
            if open {
                typst::close(&mut file).await?;
            }
            if entry.kind == "section" && !main_started {
                file.write_all(b"#counter(heading).update(0)\n").await?;
                main_started = true;
            }
            open = true;
        }
        let anchor = plan.anchor(&entry.placement, Some(&entry.fragment));
        let markdown = if entry.format == "text/markdown" {
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
        typst::entry(
            &mut file,
            plan,
            &entry,
            &anchor,
            markdown.as_ref(),
            entries.appendix,
            diagnostics,
        )
        .await?;
    }
    typst::end(&mut file, open).await?;
    if !diagnostics.is_empty() {
        file.write_all(b"#pagebreak()\n#heading(level: 1, numbering: none)[Publication diagnostics]\n").await?;
        for error in diagnostics {
            file.write_all(
                format!(
                    "#block(above: \
                     8pt)[#strong[#text({})]#linebreak()#text({})]\n",
                    super::markup::quote(&error.code),
                    super::markup::quote(&format!(
                        "{} ({})",
                        error.message, error.reference
                    ))
                )
                .as_bytes(),
            )
            .await?;
        }
    }
    file.flush().await?;
    Ok(())
}

pub(super) fn letter(mut number: u32) -> String {
    let mut output = String::new();
    while number > 0 {
        number -= 1;
        output.insert(0, (b'A' + (number % 26) as u8) as char);
        number /= 26;
    }
    output
}

pub(super) fn code_language(format: &str) -> Result<&'static str> {
    match format {
        "text/plain" => Ok("text"),
        "application/sparql-query" => Ok("sql"),
        "application/n-triples" | "text/turtle" => Ok("text"),
        _ => anyhow::bail!("unsupported fragment format {format}"),
    }
}
