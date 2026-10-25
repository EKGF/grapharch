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
    if authors.is_empty() {
        authors.push(publisher.clone());
    }
    let author = quote(&authors.join(", "));
    let mut front = String::new();
    if plan.renderer == "specification" {
        front = super::specification::pdf_metadata(plan, diagnostics).await?;
    }
    file.write_all(
        format!(
            "// Grapharch project: {}\n// Target: {}\n// Revision: {}\n// \
             Scope: {}\n// Profile: {}\n#set document(title: {}, author: \
             {author}, date: none)\n#set page(paper: \"a4\", margin: (x: \
             22mm, y: 20mm), numbering: \"1\")\n#set text(font: \"Libertinus \
             Serif\", size: 10.5pt, lang: \"en\")\n#set par(justify: \
             true)\n#set heading(numbering: \"1.1\")\n#show heading: set \
             text(fill: rgb(\"#10243a\"))\n#show table.cell.where(y: 0): set \
             text(weight: \"bold\")\n#show raw: set text(font: \"DejaVu Sans \
             Mono\", size: 8.5pt)\n#text(size: 13pt, fill: \
             rgb(\"#526174\"))[#text({})]\n#v(8pt)\n#text(size: 28pt, weight: \
             \"bold\")[#text({})]\n#v(12pt)\n{front}\n#v(20pt)\n#\
             outline(title: [Contents], depth: 3)\n#pagebreak()\n",
            plan.project,
            plan.target,
            plan.revision,
            plan.scope,
            plan.profile,
            quote(&plan.title),
            quote(&publisher),
            quote(&plan.title)
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
                "#heading(level: 1{numbering})[#text({})]#label({})\n",
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
