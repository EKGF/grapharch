use {
    super::{
        content::{Entry, code_language},
        markup::{Markdown, quote},
        model::{Plan, title},
        typst_markdown,
    },
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
pub(super) async fn begin_book(
    file: &mut BufWriter<File>,
    plan: &Plan,
    book_title: &str,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<()> {
    let mut authors = Vec::new();
    let mut rows = plan.source.objects(&plan.target, "author").await?;
    while let Some(row) = rows.next().await? {
        anyhow::ensure!(authors.len() < 64, "PDF author budget exceeded");
        if let oxrdf::Term::NamedNode(person) = &row["value"] {
            authors.push(title(&plan.source, person).await?);
        } else {
            anyhow::bail!("PDF author must be an IRI");
        }
    }
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
    let author = quote(&authors.join(", "));
    let mut front = String::new();
    if matches!(
        plan.renderer.as_str(),
        "specification" | "bookshelf"
    ) {
        front = super::specification::pdf_metadata(plan, diagnostics).await?;
    }
    let style = super::typst_style::Style::read(plan).await?;
    file.write_all(
        format!(
            "// Grapharch project: {}\n// Target: {}\n// Revision: {}\n// \
             Scope: {}\n// Profile: {}\n#set document(title: {}, author: \
             {author}, date: none)\n#let ga-style = {}\n#let ga-accent = \
             rgb({})\n#let ga-title = {}\n#let ga-publisher = {}\n#let \
             ga-ribbon = {ribbon}\n",
            plan.project,
            plan.target,
            plan.revision,
            plan.scope,
            plan.profile,
            quote(book_title),
            quote(style.name),
            quote(&style.accent),
            quote(book_title),
            quote(&publisher)
        )
        .as_bytes(),
    )
    .await?;
    file.write_all(include_str!("typst/theme.typ").as_bytes())
        .await?;
    let subtitle = match plan.source.optional(&plan.target, "subtitle").await? {
        Some(oxrdf::Term::Literal(value)) => {
            format!(
                "#v(5mm)\n#text(size: 16pt, fill: ga-muted)[#text({})]\n",
                quote(value.value())
            )
        },
        None => String::new(),
        _ => anyhow::bail!("publication subtitle must be a literal"),
    };
    if style.name == "reader" {
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
    let toc_depth = if plan.renderer == "bookshelf" {
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
        plan.toc_depth().await?.unwrap_or(3)
    };
    file.write_all(
        format!(
            "\n#ga-cover(ga-publisher, ga-title, [{subtitle}], \
             [{front}])\n#pagebreak()\n#outline(title: [Contents], depth: \
             {toc_depth})\n#pagebreak()\n#counter(heading).update(0)\n"
        )
        .as_bytes(),
    )
    .await?;
    Ok(())
}

pub(super) async fn entry(
    file: &mut BufWriter<File>,
    plan: &Plan,
    entry: &Entry,
    anchor: &str,
    markdown: Option<&Markdown<'_>>,
    appendix: u32,
    diagnostics: &[crate::publication::diagnostics::Diagnostic],
) -> Result<()> {
    if entry.first {
        if matches!(
            entry.kind.as_str(),
            "appendix" | "glossary" | "references"
        ) {
            file.write_all(b"#pagebreak(weak: true)\n").await?;
        }
        file.write_all(b"#[\n").await?;
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
        let numbering = if matches!(
            entry.kind.as_str(),
            "glossary" | "references" | "abstract" | "status"
        ) {
            ", numbering: none"
        } else {
            ""
        };
        file.write_all(
            format!(
                "#heading(level: {}{numbering})[#text({})]#label({})\n",
                entry.depth,
                quote(&entry.title),
                quote(&plan.anchor(&entry.placement, None))
            )
            .as_bytes(),
        )
        .await?;
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
        format!(
            "#raw({}, block: true, lang: {})\n",
            quote(&entry.body),
            quote(code_language(&entry.format)?)
        )
    };
    file.write_all(
        format!(
            "#block(breakable: true)[{body}]#label({})\n",
            quote(anchor)
        )
        .as_bytes(),
    )
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
