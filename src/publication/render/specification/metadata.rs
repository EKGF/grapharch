use {
    super::super::{markup::html_escape, model::Plan},
    anyhow::{Result, ensure},
    chrono::NaiveDate,
    oxrdf::{NamedNode, Term},
};

pub(super) const URL_FIELDS: &[(&str, &str, bool)] = &[
    ("thisVersion", "This version:", true),
    ("latestVersion", "Latest published version:", true),
    ("previousVersion", "Previous version:", false),
    ("editorDraft", "Latest editor's draft:", false),
    (
        "implementationReport",
        "Implementation report:",
        false,
    ),
    ("testSuite", "Test suite:", false),
    (
        "latestRecommendation",
        "Latest Recommendation:",
        false,
    ),
];

pub(in crate::publication::render) struct Header {
    pub html:  String,
    pub color: String,
}

pub(in crate::publication::render) async fn header(
    plan: &Plan,
) -> Result<Header> {
    let document_title = explicit_title(plan, &plan.target).await?;
    let subtitle = optional(plan, &plan.target, "subtitle")
        .await?
        .map(|subtitle| {
            format!(
                "<h2 class=\"subtitle\">{}</h2>",
                html_escape(&subtitle)
            )
        })
        .unwrap_or_default();
    let status = required(plan, &plan.target, "publicationStatus").await?;
    let date = match plan
        .source
        .optional(&plan.target, "publicationDate")
        .await?
    {
        Some(Term::Literal(date))
            if date.datatype().as_str() ==
                "http://www.w3.org/2001/XMLSchema#date" =>
        {
            date
        },
        _ => anyhow::bail!("publicationDate must be an xsd:date"),
    };
    let parsed =
        NaiveDate::parse_from_str(date.value(), "%Y-%m-%d").map_err(|_| {
            anyhow::anyhow!("publicationDate must be a valid YYYY-MM-DD date")
        })?;
    ensure!(
        parsed.format("%Y-%m-%d").to_string() == date.value(),
        "publicationDate must use YYYY-MM-DD"
    );
    let authority = super::authority::Authority::read(plan).await?;
    let status_html = if let Some(url) =
        plan.source.optional(&plan.target, "statusUrl").await?
    {
        let Term::NamedNode(url) = url else {
            anyhow::bail!("statusUrl must be an HTTP(S) IRI")
        };
        link(
            url.as_str(),
            &format!("{} {}", authority.short, status),
        )?
    } else {
        format!("{} {}", authority.link, html_escape(&status))
    };
    let disclosure = super::config::flag(plan, "metadataDisclosure").await?;
    let metadata_open = if disclosure {
        "<details class=\"publication-details\" open><summary>More details \
         about this document</summary>"
    } else {
        ""
    };
    let product_logo =
        match plan.source.optional(&plan.target, "productLogo").await? {
            Some(Term::NamedNode(asset)) => {
                format!(
                    "<img class=\"product-logo\" src=\"{}\" alt=\"{}\" \
                     width=\"100\">",
                    html_escape(plan.images.asset_route(&asset)?),
                    html_escape(&explicit_title(plan, &asset).await?),
                )
            },
            None => String::new(),
            _ => anyhow::bail!("productLogo must be an asset IRI"),
        };
    let logos = format!("{}{product_logo}", authority.logo);
    let banner = format!("{} {}", authority.short, status);
    let mut html = format!(
        "<div class=\"authority-banner\" aria-label=\"{}\">{}</div><header \
         class=\"head\" style=\"--authority-color:{}\" \
         data-authority=\"{}\">{}<h1>{}</h1>{subtitle}<p \
         class=\"publication-status\">{status_html} <time \
         datetime=\"{}\">{}</time></p><a class=\"pdf-download\" \
         href=\"document.pdf\" type=\"application/pdf\" aria-label=\"Download \
         PDF\"><svg aria-hidden=\"true\" focusable=\"false\" width=\"26\" \
         height=\"30\" viewBox=\"0 0 28 32\"><path d=\"M5 1h13l8 8v22H5zM18 \
         1v8h8\" fill=\"white\" stroke=\"currentColor\" \
         stroke-width=\"1.5\"/><rect x=\"0\" y=\"17\" width=\"25\" \
         height=\"12\" rx=\"2\" fill=\"#c62828\"/><text x=\"12.5\" y=\"26\" \
         text-anchor=\"middle\" fill=\"white\" \
         font-family=\"Arial,sans-serif\" font-weight=\"bold\" \
         font-size=\"9\">PDF</text></svg><span>PDF</span></\
         a>{metadata_open}<dl class=\"publication-metadata\">",
        html_escape(&banner),
        html_escape(&banner),
        authority.color,
        html_escape(&authority.name),
        logos,
        html_escape(&document_title),
        html_escape(date.value()),
        parsed.format("%-d %B %Y")
    );
    let mut defaults = String::new();
    for (field, label) in [
        ("releaseVersion", "Version"),
        ("documentNumber", "Document identifier"),
    ] {
        if let Some(value) = optional(plan, &plan.target, field).await? {
            html.push_str(&format!(
                "<dt>{label}:</dt><dd>{}</dd>",
                html_escape(&value)
            ));
        }
    }
    for &(field, label, required) in URL_FIELDS {
        match plan.source.optional(&plan.target, field).await? {
            Some(Term::NamedNode(url)) => {
                defaults.push_str(&format!(
                    "<dt>{label}</dt><dd>{}</dd>",
                    link(url.as_str(), url.as_str())?
                ));
            },
            None if !required => {},
            _ => anyhow::bail!("{field} must be an HTTP(S) IRI"),
        }
    }
    if let Some(rows) = super::rows::render(plan).await? {
        html.push_str(&rows);
    } else {
        html.push_str(&defaults);
        for (role, singular, plural) in [
            ("editor", "Editor", "Editors"),
            ("author", "Author", "Authors"),
            ("formerEditor", "Former editor", "Former editors"),
        ] {
            html.push_str(&people(plan, role, singular, plural, None).await?);
        }
    }
    html.push_str("</dl>");
    if disclosure {
        html.push_str("</details>");
    }
    if let Some(copyright) = optional(plan, &plan.target, "copyright").await? {
        html.push_str(&format!(
            "<p class=\"copyright\">{}</p>",
            html_escape(&copyright)
        ));
    }
    if let Some(license) = plan.source.optional(&plan.target, "license").await?
    {
        let Term::NamedNode(license) = license else {
            anyhow::bail!("license must be an HTTP(S) IRI")
        };
        validate_url(license.as_str())?;
        html.push_str(&format!(
            "<p class=\"copyright\"><a rel=\"license\" href=\"{}\">Document \
             license</a></p>",
            html_escape(license.as_str())
        ));
    }
    html.push_str("</header>\n");
    html.push_str(&super::super::preamble::html(plan).await?);
    Ok(Header { html, color: authority.color })
}

pub(super) async fn people(
    plan: &Plan,
    role: &str,
    singular: &str,
    plural: &str,
    label: Option<&str>,
) -> Result<String> {
    let mut editors = Vec::new();
    let mut rows = plan.source.objects(&plan.target, role).await?;
    while let Some(row) = rows.next().await? {
        ensure!(
            editors.len() < 64,
            "specification exceeds 64 people in one role"
        );
        let Term::NamedNode(editor) = &row["value"] else {
            anyhow::bail!("publication person must be an IRI")
        };
        let name = explicit_title(plan, editor).await?;
        let display = if let Some(url) = optional(plan, editor, "url").await? {
            link(&url, &name)?
        } else {
            html_escape(&name)
        };
        let affiliation = optional(plan, editor, "affiliation").await?;
        let display = if let Some(affiliation) = affiliation {
            format!("{display} ({})", html_escape(&affiliation))
        } else {
            display
        };
        let mut display = display;
        if let Some(note) = optional(plan, editor, "personNote").await? {
            display.push_str(&format!(" {}", html_escape(&note)));
        }
        if let Some(orcid) = plan.source.optional(editor, "orcid").await? {
            let Term::NamedNode(orcid) = orcid else {
                anyhow::bail!("orcid must be an HTTP(S) IRI")
            };
            display.push_str(&format!(
                " <span class=\"orcid\">{}</span>",
                link(orcid.as_str(), "ORCID")?
            ));
        }
        let order = plan
            .source
            .optional(editor, "order")
            .await?
            .map(|value| {
                crate::publication::validation::unsigned(&value, "person order")
            })
            .transpose()?;
        editors.push((order, name, editor.to_string(), display));
    }
    if editors.iter().any(|(order, _, _, _)| order.is_some()) {
        ensure!(
            editors.iter().all(|(order, _, _, _)| order.is_some()),
            "all people in an ordered role require an explicit order"
        );
        let mut orders = std::collections::HashSet::new();
        ensure!(
            editors.iter().all(|(order, _, _, _)| orders.insert(*order)),
            "people in a role require distinct orders"
        );
    }
    editors.sort();
    let mut html = String::new();
    if !editors.is_empty() {
        let heading = label.map(str::to_owned).unwrap_or_else(|| {
            format!(
                "{}:",
                if editors.len() == 1 { singular } else { plural }
            )
        });
        html.push_str(&format!("<dt>{}</dt>", html_escape(&heading)));
        for (_, _, _, display) in editors {
            html.push_str(&format!("<dd>{display}</dd>"));
        }
    }
    Ok(html)
}

pub(super) async fn explicit_title(
    plan: &Plan,
    node: &NamedNode,
) -> Result<String> {
    let predicate = NamedNode::new_unchecked("http://purl.org/dc/terms/title");
    match plan.source.optional_by(node, &predicate).await? {
        Some(Term::Literal(title))
            if !title.value().trim().is_empty() &&
                title.value().len() <= 4096 =>
        {
            Ok(title.value().to_owned())
        },
        _ => anyhow::bail!("{node} requires a nonempty dcterms:title"),
    }
}

async fn required(
    plan: &Plan,
    node: &NamedNode,
    field: &str,
) -> Result<String> {
    optional(plan, node, field)
        .await?
        .ok_or_else(|| anyhow::anyhow!("missing {field}"))
}

pub(super) async fn optional(
    plan: &Plan,
    node: &NamedNode,
    field: &str,
) -> Result<Option<String>> {
    match plan.source.optional(node, field).await? {
        Some(Term::Literal(value)) if !value.value().trim().is_empty() => {
            ensure!(
                value.value().len() <= 4096,
                "{field} exceeds metadata length limit"
            );
            Ok(Some(value.value().to_owned()))
        },
        None => Ok(None),
        _ => anyhow::bail!("{field} requires a nonempty literal"),
    }
}

pub(super) fn validate_url(value: &str) -> Result<()> {
    ensure!(
        value.len() <= 4096,
        "publication URL exceeds metadata length limit"
    );
    let url = url::Url::parse(value)?;
    ensure!(
        matches!(url.scheme(), "https" | "http") &&
            url.host_str().is_some() &&
            url.username().is_empty() &&
            url.password().is_none(),
        "publication links require absolute HTTP(S) URLs without credentials"
    );
    Ok(())
}

pub(super) fn link(url: &str, label: &str) -> Result<String> {
    validate_url(url)?;
    Ok(format!(
        "<a href=\"{}\">{}</a>",
        html_escape(url),
        html_escape(label)
    ))
}
