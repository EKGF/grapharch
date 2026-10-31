//! Pinned, nonfatal review findings shared by publication formats.
mod evidence;
mod presentation;
pub(super) use evidence::EvidenceIndex;
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
    dataset: &crate::publication::dataset::Dataset,
) -> Result<(Vec<Diagnostic>, EvidenceIndex)> {
    let mut rows = view.objects(plan, "diagnostic").await?;
    let mut identities = BTreeMap::new(); // bounded: 1024 diagnostic identities.
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
        let issue = Diagnostic {
            severity: Severity::Warning,
            code: view.text(id, "code").await?,
            message: view.text(id, "message").await?,
            reference: view.text(id, "unresolvedReference").await?,
            source_fragment,
        };
        identities.insert(id.clone(), evidence::key(&issue));
        result.push(issue);
    }
    result.sort_by(|a, b| {
        a.reference
            .cmp(&b.reference)
            .then(a.message.cmp(&b.message))
    });
    let evidence =
        EvidenceIndex::read(dataset, view, plan, &identities).await?;
    for issue in &result {
        ensure!(
            issue.code != "competing-source-annotations" ||
                !evidence.get(issue).is_empty(),
            "competing annotation diagnostic has no structured evidence"
        );
    }
    Ok((result, evidence))
}

pub(super) fn inline(
    body: &str,
    subject: &NamedNode,
    fragment: &NamedNode,
    issues: &[Diagnostic],
    evidence: &EvidenceIndex,
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
        let entries = evidence.get(issue);
        if entries.is_empty() {
            for line in issue.message.lines() {
                result.push_str("> ");
                result.push_str(&super::markup::html_escape(line));
                result.push('\n');
            }
        } else {
            result.push_str(&presentation::markdown(entries));
        }
        result.push('\n');
    }
    result.push_str(body);
    result
}

struct Group<'a> {
    code:   String,
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
                Group {
                    code:   issue.code.clone(),
                    title:  title.into(),
                    issues: Vec::new(),
                }
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
    evidence: &EvidenceIndex,
) -> Result<()> {
    html_titled(file, issues, "Quality issues", evidence).await
}

pub(super) async fn html_titled(
    file: &mut (impl AsyncWrite + Unpin),
    issues: &[Diagnostic],
    title: &str,
    evidence: &EvidenceIndex,
) -> Result<()> {
    if issues.is_empty() {
        return Ok(())
    }
    file.write_all(
        format!(
            "<section id=\"quality-issues\">{}<p>Review findings preserve \
             competing source statements; they are not automatically RDF \
             errors.</p>",
            super::markup::structural_heading(2, title, "quality-issues")
        )
        .as_bytes(),
    )
    .await?;
    for group in groups(issues) {
        file.write_all(
            format!(
                "<section class=\"quality-issue-group\">{}<ul>",
                super::markup::structural_heading(
                    3,
                    &group.title,
                    &format!("quality-{}", group.code)
                )
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
            let entries = evidence.get(issue);
            if !entries.is_empty() {
                file.write_all(presentation::html(entries).as_bytes())
                    .await?;
            } else if group.detail(issue) {
                file.write_all(
                    format!(
                        "<p>{}</p>",
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
    evidence: &EvidenceIndex,
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
                    super::markup::quote(&presentation::wrappable(
                        &issue.reference
                    ))
                )
                .as_bytes(),
            )
            .await?;
            let entries = evidence.get(issue);
            if !entries.is_empty() {
                file.write_all(presentation::typst(entries).as_bytes())
                    .await?;
            } else if group.detail(issue) {
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
                &[issue],
                &super::EvidenceIndex::default()
            ),
            body
        );
    }
}
