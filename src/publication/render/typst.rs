use {
    super::{
        content::{Entry, code_language},
        markup::{Markdown, quote},
        model::{Plan, title},
        typst_markdown,
    },
    crate::publication::theme::{Builtin, PdfTheme},
    anyhow::Result,
    tokio::{
        fs::File,
        io::{AsyncWriteExt, BufWriter},
    },
};

pub(super) async fn begin(
    file: &mut BufWriter<File>,
    plan: &Plan,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<()> {
    begin_book(file, plan, &plan.title, diagnostics).await
}

/// The catalogue theme used when a PDF profile selects none.
pub(super) fn default_theme(plan: &Plan) -> Builtin {
    if matches!(
        plan.renderer.as_str(),
        "specification" | "bookshelf"
    ) {
        Builtin::Specification
    } else {
        Builtin::Reader
    }
}

pub(super) async fn theme(plan: &Plan) -> Result<PdfTheme> {
    PdfTheme::read(&plan.source, &plan.profile, default_theme(plan)).await
}

async fn accent(plan: &Plan, theme: &PdfTheme) -> Result<String> {
    let fallback = theme.text("accent").unwrap_or_default();
    if !theme.flag("authorityAccent") {
        return Ok(fallback);
    }
    match plan.source.optional(&plan.target, "publisher").await? {
        Some(oxrdf::Term::NamedNode(publisher)) => {
            match plan.source.optional(&publisher, "accentColor").await? {
                Some(oxrdf::Term::Literal(color)) => {
                    let color = color.value();
                    anyhow::ensure!(
                        color.len() == 7 &&
                            color.starts_with('#') &&
                            color[1..]
                                .bytes()
                                .all(|c| c.is_ascii_hexdigit()),
                        "PDF accentColor must be #RRGGBB"
                    );
                    Ok(color.to_owned())
                },
                None => Ok(fallback),
                _ => anyhow::bail!("authority accentColor must be a literal"),
            }
        },
        None => Ok(fallback),
        _ => anyhow::bail!("PDF publisher must be an IRI"),
    }
}

pub(super) async fn begin_book(
    file: &mut BufWriter<File>,
    plan: &Plan,
    book_title: &str,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<()> {
    let mut authors = super::people::read(plan, "author")
        .await?
        .into_iter()
        .map(|person| person.name)
        .collect::<Vec<_>>();
    let publisher =
        match plan.source.optional(&plan.target, "publisher").await? {
            Some(oxrdf::Term::NamedNode(publisher)) => {
                crate::publication::governance::name(&plan.source, &publisher)
                    .await?
            },
            None => title(&plan.source, &plan.project).await?,
            _ => anyhow::bail!("PDF publisher must be an IRI"),
        };
    let ribbon = match plan
        .source
        .optional(&plan.target, "publicationStatus")
        .await?
    {
        Some(oxrdf::Term::Literal(status)) => {
            anyhow::ensure!(
                !status.value().trim().is_empty(),
                "PDF publicationStatus cannot be empty"
            );
            let authority =
                match plan.source.optional(&plan.target, "publisher").await? {
                    Some(oxrdf::Term::NamedNode(publisher)) => {
                        crate::publication::governance::acronym(
                            &plan.source,
                            &publisher,
                        )
                        .await?
                    },
                    None => publisher.clone(),
                    _ => anyhow::bail!("PDF publisher must be an IRI"),
                };
            quote(&format!("{authority} {}", status.value()))
        },
        None => "none".into(),
        _ => anyhow::bail!("PDF publicationStatus must be a literal"),
    };
    let supplied_authors = authors.clone();
    if authors.is_empty() {
        authors.push(publisher.clone());
    }
    let metadata = super::document_metadata::DocumentMetadata::read(
        plan, book_title, authors,
    )
    .await?
    .typst();
    let conformance = super::pdf_standard::PdfConformance::read(plan)
        .await?
        .typst();
    let theme = theme(plan).await?;
    let mut front = String::new();
    let mut release = String::new();
    for (field, label) in [
        ("releaseVersion", "Version"),
        ("documentNumber", "Document identifier"),
    ] {
        if let Some(value) = plan.source.optional(&plan.target, field).await? {
            let oxrdf::Term::Literal(value) = value else {
                anyhow::bail!("PDF {field} must be a literal")
            };
            release.push_str(&format!(
                "#block(above: 5pt)[#strong[#text({})] #text({})]\n",
                quote(&format!("{label}:")),
                quote(value.value())
            ));
        }
    }
    let specification_cover = theme.flag("specificationCover");
    let specification_metadata = matches!(
        plan.renderer.as_str(),
        "specification" | "bookshelf"
    ) || (specification_cover &&
        plan.source
            .optional(&plan.target, "publicationStatus")
            .await?
            .is_some());
    if specification_metadata {
        front = super::specification::pdf_metadata(plan, diagnostics).await?;
    } else {
        front.insert_str(0, &release);
    }
    let footer_text =
        match plan.source.optional(&plan.target, "footerText").await? {
            Some(oxrdf::Term::Literal(value)) => value.value().to_owned(),
            None => book_title.to_owned(),
            _ => anyhow::bail!("ga:footerText must be a literal"),
        };
    file.write_all(
        format!(
            "// Grapharch project: {}\n// Target: {}\n// Revision: {}\n// \
             Scope: {}\n// Profile: {}\n{metadata}#let ga-accent = \
             rgb({})\n{}#let ga-title = {}\n#let ga-publisher = {}\n#let \
             ga-footer-text = {}\n#let ga-ribbon = {ribbon}\n#let \
             ga-diagram-note = {}\n",
            plan.project,
            plan.target,
            plan.revision,
            plan.scope,
            plan.profile,
            quote(&accent(plan, &theme).await?),
            theme.typst()?,
            quote(book_title),
            quote(&publisher),
            quote(&footer_text),
            plan.profile_flag("diagramNote").await?
        )
        .as_bytes(),
    )
    .await?;
    file.write_all(include_str!("typst/theme.typ").as_bytes())
        .await?;
    let subtitle = match plan.source.optional(&plan.target, "subtitle").await? {
        Some(oxrdf::Term::Literal(value)) => {
            Some(format!("#text({})", quote(value.value())))
        },
        None => None,
        _ => anyhow::bail!("publication subtitle must be a literal"),
    };
    if !specification_cover && !specification_metadata {
        if let Some(value) = plan
            .source
            .optional_by(
                &plan.target,
                &oxrdf::NamedNode::new("http://purl.org/dc/terms/description")?,
            )
            .await?
        {
            let oxrdf::Term::Literal(value) = value else {
                anyhow::bail!("PDF description must be a literal")
            };
            front.push_str(&format!(
                "#block(above: 8pt, below: 16pt)[#text(size: 12pt, fill: \
                 ga-muted)[#text({})]]\n",
                quote(value.value())
            ));
        }
        for (field, label) in [
            ("publicationStatus", "Status"),
            ("publicationDate", "Published"),
        ] {
            if let Some(value) =
                plan.source.optional(&plan.target, field).await?
            {
                let oxrdf::Term::Literal(value) = value else {
                    anyhow::bail!("PDF {field} must be a literal")
                };
                front.push_str(&format!(
                    "#block(above: 8pt)[#strong[#text({})] #text({})]\n",
                    quote(&format!("{label}:")),
                    quote(value.value())
                ));
            }
        }
        if !supplied_authors.is_empty() {
            front.push_str(&format!(
                "#block(above: 8pt)[#strong[Authors:] #text({})]\n",
                quote(&supplied_authors.join(", "))
            ));
        }
    }
    let toc_depth = if plan.renderer == crate::publication::Renderer::Bookshelf
    {
        match plan.source.optional(&plan.target, "pdfTocDepth").await? {
            None => 2,
            Some(oxrdf::Term::Literal(value))
                if value.datatype().as_str() ==
                    "http://www.w3.org/2001/XMLSchema#integer" =>
            {
                let depth = value.value().parse::<usize>()?;
                anyhow::ensure!(
                    (1..=8).contains(&depth),
                    "pdfTocDepth must be between one and eight"
                );
                depth
            },
            _ => anyhow::bail!("pdfTocDepth must be an xsd:integer"),
        }
    } else {
        match plan.toc_depth().await? {
            Some(depth) => depth,
            None => theme.depth("contentsDepth")?,
        }
    };
    let cover = super::cover::typst(
        plan,
        &theme,
        &super::cover::CoverContent {
            publisher: publisher.clone(),
            title: book_title.to_owned(),
            subtitle,
            summary: front,
        },
        diagnostics,
    )
    .await?;
    let notices = super::notices::typst(plan, diagnostics).await?;
    let front_numbering = match theme.text("frontNumbering") {
        Some(numbering) => {
            format!("#set page(numbering: {})\n", quote(&numbering))
        },
        None => String::new(),
    };
    let toc_title = quote(&theme.text("tocTitle").unwrap_or_default());
    let front = format!(
        "\n{conformance}{front_numbering}{cover}#pagebreak()\n{notices}{}",
        concat!(
            "#[\n",
            "#show heading: set text(font: ga-theme.toc_title_font, ",
            "size: ga-theme.toc_title_size, weight: \"bold\")\n",
            "#show heading: set align(ga-theme.toc_title_align)\n",
        ),
    ) + &format!(
        "#heading(level: 1, numbering: none, outlined: \
         false)[#text({toc_title})]\n]\n#outline(title: none, depth: \
         {toc_depth})\n#pagebreak()\n#counter(heading).update(0)\n"
    );
    file.write_all(front.as_bytes()).await?;
    Ok(())
}

/// Arabic body numbering starts on the page of the first numbered chapter.
pub(super) async fn body_numbering(plan: &Plan) -> Result<String> {
    Ok(match theme(plan).await?.text("bodyNumbering") {
        Some(numbering) => {
            format!(
                "#pagebreak(weak: true)\n#counter(page).update(1)\n#set \
                 page(numbering: {})\n",
                quote(&numbering)
            )
        },
        None => String::new(),
    })
}

/// Theme decisions the per-entry writer needs, read once per document.
pub(super) struct EntryStyle {
    chapter_break: bool,
    annex_label:   Option<String>,
    normativity:   bool,
    centered:      bool,
    recto:         Option<String>,
}

impl EntryStyle {
    pub(super) async fn read(plan: &Plan) -> Result<Self> {
        let theme = theme(plan).await?;
        Ok(Self {
            chapter_break: theme.flag("chapterPageBreak"),
            annex_label:   theme.text("annexLabel"),
            normativity:   theme.flag("annexNormativity"),
            centered:      theme.flag("annexCentered"),
            recto:         theme
                .flag("annexRectoStart")
                .then(|| theme.text("blankPageText"))
                .flatten(),
        })
    }
}

pub(super) async fn entry(
    file: &mut BufWriter<File>,
    plan: &Plan,
    entry: &Entry,
    anchor: &str,
    markdown: Option<&Markdown<'_>>,
    (appendix, style): (u32, &EntryStyle),
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<()> {
    if entry.first {
        let annex = entry.kind == "appendix" && entry.depth == 1;
        // Page breaks must precede the fragment's layout container. Typst
        // forbids page breaks inside a heading show rule in that container.
        if annex && let Some(notice) = &style.recto {
            file.write_all(
                format!(
                    "#metadata(none)<ga-recto-{appendix}>\n#\
                     ga-blank-page(<ga-recto-{appendix}>)[#text({})]\n",
                    quote(notice)
                )
                .as_bytes(),
            )
            .await?;
        } else if entry.depth == 1 &&
            style.chapter_break &&
            !matches!(entry.kind.as_str(), "abstract" | "status")
        {
            file.write_all(b"#pagebreak(weak: true)\n").await?;
        }
        file.write_all(b"#[\n").await?;
        if entry.kind == "preface" {
            file.write_all(b"#set heading(numbering: none)\n").await?;
        }
        if entry.kind == "appendix" {
            file.write_all(
                format!(
                    "#set heading(numbering: \
                     \"A.1\")\n#counter(heading).update({})\n",
                    appendix - 1
                )
                .as_bytes(),
            )
            .await?;
        }
        file.write_all(format!("#ga-section({})[", entry.depth).as_bytes())
            .await?;
        let labelled = annex.then_some(style.annex_label.as_ref()).flatten();
        let numbering = if labelled.is_some() ||
            matches!(
                entry.kind.as_str(),
                "glossary" | "references" | "abstract" | "status" | "preface"
            ) {
            ", numbering: none"
        } else {
            ""
        };
        let heading_title = match labelled {
            Some(label) => {
                label
                    .replace("{letter}", &super::content::letter(appendix))
                    .replace("{title}", &entry.title)
            },
            None => entry.title.clone(),
        };
        let heading = format!(
            "#heading(level: {}{numbering})[#text({})]#label({})",
            entry.depth,
            quote(&heading_title),
            quote(&plan.anchor(&entry.placement, None))
        );
        let normativity = if annex && style.normativity {
            match plan
                .source
                .optional(&entry.placement, "normativity")
                .await?
            {
                Some(oxrdf::Term::Literal(value)) => {
                    format!(
                        "#block(below: 18pt)[#text(font: \
                         ga-theme.heading_font, size: \
                         ga-theme.heading_2_size)[#text({})]]",
                        quote(&format!("({})", value.value()))
                    )
                },
                None => {
                    anyhow::bail!(
                        "annex placement {} requires ga:normativity",
                        entry.placement
                    )
                },
                _ => anyhow::bail!("ga:normativity must be a literal"),
            }
        } else {
            String::new()
        };
        if annex && style.centered {
            file.write_all(
                format!("#align(center)[{heading}{normativity}]\n").as_bytes(),
            )
            .await?;
        } else {
            file.write_all(format!("{heading}{normativity}\n").as_bytes())
                .await?;
        }
        if labelled.is_some() {
            file.write_all(
                format!("#counter(heading).update({appendix})\n").as_bytes(),
            )
            .await?;
        }
    }
    if !entry.first {
        if let Some(markdown) = markdown &&
            let Some(pulldown_cmark::Event::Start(
                pulldown_cmark::Tag::Heading { level, .. },
            )) = markdown.events.first()
        {
            let level = markdown.depth(*level) + markdown.component_depth;
            file.write_all(format!("#ga-section({level})[").as_bytes())
                .await?;
        } else {
            file.write_all(b"#block(breakable: true)[").await?;
        }
    }
    for field in ["nativeAnchor", "headingAnchor"] {
        if let Some(oxrdf::Term::Literal(alias)) =
            plan.source.optional(&entry.fragment, field).await?
        {
            file.write_all(
                format!(
                    "#metadata({})#label({})",
                    quote(alias.value()),
                    quote(alias.value())
                )
                .as_bytes(),
            )
            .await?;
        }
    }
    let body = if entry.format == "text/html" {
        super::native_typst::convert(&entry.body, diagnostics)?
    } else if let Some(markdown) = markdown {
        typst_markdown::convert(markdown)?
    } else {
        super::code::typst(&entry.body, code_language(&entry.format)?)?
    };
    file.write_all(format!("{body}]#label({})\n", quote(anchor)).as_bytes())
        .await?;
    Ok(())
}

pub(super) async fn end(
    file: &mut BufWriter<File>,
    has_entries: bool,
) -> Result<()> {
    if has_entries {
        close(file).await?;
    }
    Ok(())
}
pub(super) async fn close(file: &mut BufWriter<File>) -> Result<()> {
    file.write_all(b"]\n").await?;
    Ok(())
}
