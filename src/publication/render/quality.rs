//! Pinned, nonfatal review findings shared by publication formats.
use {
    crate::publication::{
        diagnostics::{Diagnostic, Severity},
        view::RevisionView,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
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

pub(super) async fn html(
    file: &mut (impl AsyncWrite + Unpin),
    issues: &[Diagnostic],
) -> Result<()> {
    if issues.is_empty() {
        return Ok(())
    }
    file.write_all(b"<section id=\"quality-issues\"><h2>Quality issues</h2><p>Review findings preserve competing source statements; they are not automatically RDF errors.</p><ol>").await?;
    for issue in issues {
        file.write_all(
            format!(
                "<li><strong>{}</strong><pre>{}</pre></li>",
                super::markup::html_escape(&issue.reference),
                super::markup::html_escape(&issue.message)
            )
            .as_bytes(),
        )
        .await?;
    }
    file.write_all(b"</ol></section>\n").await?;
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
