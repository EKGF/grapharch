//! Keep source navigation valid when a file repeats already-documented
//! subjects.
use {
    crate::publication::{
        collect::{Collector, GeneratedPage, MarkdownFile, page::slug},
        dataset::identity,
        writer::GraphWriter,
    },
    anyhow::Result,
    oxrdf::NamedNode,
    std::collections::BTreeSet,
};

pub(super) async fn collect(
    writer: &GraphWriter,
    project: &NamedNode,
    path: &str,
    owners: BTreeSet<String>,
) -> Result<()> {
    let id = identity("quick-rdf-alias", &[project.as_str(), path]);
    let mut body = format!(
        "# {path}\n\nThe resources in this source are documented in:\n\n"
    );
    for owner in owners {
        body.push_str(&format!("- [{owner}](/{owner})\n"));
    }
    MarkdownFile::generated(
        project.as_str(),
        "urn:grapharch:quick-rdf-alias",
        GeneratedPage {
            source: path,
            id: &id,
            path,
            title: path,
            route: &format!("reference/{}/", slug(path)),
            body,
            revision: "quick-alias-v1".into(),
        },
    )?
    .collect(writer)
    .await
}
