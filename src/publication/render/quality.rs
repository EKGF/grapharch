//! Pinned, nonfatal review findings shared by publication formats.
use {
    crate::publication::{
        diagnostics::{Diagnostic, Severity},
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
    tokio::io::{AsyncWrite, AsyncWriteExt},
};

pub(super) async fn read(
    view: &RevisionView,
    plan: &NamedNode,
) -> Result<Vec<Diagnostic>> {
    let mut rows = view.objects(plan, "diagnostic").await?;
    let mut result = Vec::new(); // bounded: 1024 pinned review findings.
    while let Some(row) = rows.next().await? {
        ensure!(
            result.len() < 1024,
            "quality issue budget exceeded"
        );
        let Term::NamedNode(id) = &row["value"] else {
            anyhow::bail!("quality diagnostic must be an IRI")
        };
        ensure!(
            view.text(id, "severity").await? == "warning",
            "quality diagnostics must be warnings"
        );
        let source_fragment = match view.optional(id, "sourceFragment").await? {
            Some(Term::NamedNode(fragment)) => Some(fragment.into_string()),
            None => None,
            _ => anyhow::bail!("quality source fragment must be an IRI"),
        };
        result.push(Diagnostic {
            severity: Severity::Warning,
            code: view.text(id, "code").await?,
            message: view.text(id, "message").await?,
            reference: view.text(id, "unresolvedReference").await?,
            source_fragment,
        });
    }
    result.sort_by(|a, b| {
        a.reference
            .cmp(&b.reference)
            .then(a.message.cmp(&b.message))
    });
    Ok(result)
}

pub(super) fn inline(
    body: &str,
    subject: &NamedNode,
    fragment: &NamedNode,
    issues: &[Diagnostic],
) -> String {
    let mut result = String::new();
    for issue in issues.iter().filter(|issue| {
        issue.reference == subject.as_str() &&
            !(matches!(
                issue.code.as_str(),
                "missing-source-label" | "missing-source-definition"
            ) && issue.source_fragment.as_deref() ==
                Some(fragment.as_str()))
    }) {
        result.push_str("> [!WARNING]\n> Source quality review\n>\n");
        for line in issue.message.lines() {
            result.push_str(">     ");
            result.push_str(line);
            result.push('\n');
        }
        result.push('\n');
    }
    result.push_str(body);
    result
}

struct Group<'a> {
    title:  String,
    issues: Vec<&'a Diagnostic>,
}

fn groups(issues: &[Diagnostic]) -> Vec<Group<'_>> {
    // bounded: at most 1024 findings, grouped as references without cloning
    // bodies.
    let mut groups = BTreeMap::<&str, Group<'_>>::new();
    for issue in issues {
        let title = match issue.code.as_str() {
            "competing-source-annotations" => "Competing source annotations",
            "missing-source-definition" => "Missing English source definition",
            "missing-source-label" => "Missing English source label",
            _ => &issue.code,
        };
        groups
            .entry(&issue.code)
            .or_insert_with(|| {
                Group { title: title.into(), issues: Vec::new() }
            })
            .issues
            .push(issue);
    }
    groups.into_values().collect()
}

impl Group<'_> {
    fn detail(&self, issue: &Diagnostic) -> bool { issue.message != self.title }
}

pub(super) async fn html(
    file: &mut (impl AsyncWrite + Unpin),
    issues: &[Diagnostic],
) -> Result<()> {
    html_titled(file, issues, "Quality issues").await
}

pub(super) async fn html_titled(
    file: &mut (impl AsyncWrite + Unpin),
    issues: &[Diagnostic],
    title: &str,
) -> Result<()> {
    if issues.is_empty() {
        return Ok(())
    }
    file.write_all(
        format!(
            "<section id=\"quality-issues\"><h2>{}</h2><p>Review findings \
             preserve competing source statements; they are not automatically \
             RDF errors.</p>",
            super::markup::html_escape(title)
        )
        .as_bytes(),
    )
    .await?;
    for group in groups(issues) {
        file.write_all(
            format!(
                "<section class=\"quality-issue-group\"><h3>{}</h3><ul>",
                super::markup::html_escape(&group.title)
            )
            .as_bytes(),
        )
        .await?;
        for issue in &group.issues {
            file.write_all(
                format!(
                    "<li><strong>{}</strong>",
                    super::markup::html_escape(&issue.reference)
                )
                .as_bytes(),
            )
            .await?;
            if group.detail(issue) {
                file.write_all(
                    format!(
                        "<pre>{}</pre>",
                        super::markup::html_escape(&issue.message)
                    )
                    .as_bytes(),
                )
                .await?;
            }
            file.write_all(b"</li>").await?;
        }
        file.write_all(b"</ul></section>").await?;
    }
    file.write_all(b"</section>\n").await?;
    Ok(())
}

pub(super) async fn typst(
    file: &mut (impl AsyncWrite + Unpin),
    issues: &[Diagnostic],
    preceding_appendices: u32,
) -> Result<()> {
    if issues.is_empty() {
        return Ok(())
    }
    file.write_all(
        format!(
            "#pagebreak()\n#[\n#set heading(numbering: \
             \"A.1\")\n#counter(heading).update({preceding_appendices})\n#\
             heading(level: 1)[Quality issues]\n"
        )
        .as_bytes(),
    )
    .await?;
    for group in groups(issues) {
        file.write_all(
            format!(
                "#heading(level: 2, numbering: none)[#text({})]\n",
                super::markup::quote(&group.title)
            )
            .as_bytes(),
        )
        .await?;
        for issue in &group.issues {
            file.write_all(
                format!(
                    "#list([#text({})",
                    super::markup::quote(&issue.reference)
                )
                .as_bytes(),
            )
            .await?;
            if group.detail(issue) {
                file.write_all(
                    format!(
                        "#linebreak()#text({})",
                        super::markup::quote(&issue.message)
                    )
                    .as_bytes(),
                )
                .await?;
            }
            file.write_all(b"])\n").await?;
        }
    }
    file.write_all(b"]\n").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_documentor_callouts_are_not_repeated_by_rendering() {
        let subject = oxrdf::NamedNode::new_unchecked("urn:quality:subject");
        let body = "> [!WARNING]\n> Missing source definition.\n";
        let issue = crate::publication::diagnostics::Diagnostic {
            severity:        crate::publication::diagnostics::Severity::Warning,
            code:            "missing-source-definition".into(),
            message:         "Missing source definition.".into(),
            reference:       subject.as_str().into(),
            source_fragment: Some("urn:quality:fragment".into()),
        };
        assert_eq!(
            super::inline(
                body,
                &subject,
                &oxrdf::NamedNode::new_unchecked("urn:quality:fragment"),
                &[issue]
            ),
            body
        );
    }
}
