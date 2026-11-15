//! A declared negotiated path: a host serves the RDF target's files there by
//! the request's Accept header, and the named target's entry document
//! otherwise.
use {
    super::names::NegotiatedPath,
    crate::publication::view::RevisionView,
    anyhow::{Result, ensure},
    oxrdf::NamedNode,
};

#[derive(Clone, Debug)]
pub(crate) struct Negotiation {
    pub path: NegotiatedPath,
    /// Output-root path of the HTML target's entry document.
    pub html: String,
}

impl Negotiation {
    pub(super) async fn read(
        view: &RevisionView,
        project: &NamedNode,
        target: &NamedNode,
    ) -> Result<Option<Self>> {
        let path = view.optional(target, "negotiatedPath").await?;
        let html = view.optional(target, "negotiatedHtml").await?;
        let (path, html) = match (path, html) {
            (None, None) => return Ok(None),
            (
                Some(oxrdf::Term::Literal(path)),
                Some(oxrdf::Term::NamedNode(html)),
            ) => (NegotiatedPath::try_new(path.value())?, html),
            _ => {
                anyhow::bail!(
                    "RDF target {target} must declare a ga:negotiatedPath \
                     literal together with a ga:negotiatedHtml target"
                )
            },
        };
        let mut registered = false;
        let mut rows = view.objects(project, "target").await?;
        while let Some(row) = rows.next().await? {
            registered |= row["value"] == oxrdf::Term::NamedNode(html.clone());
        }
        ensure!(
            registered,
            "negotiated HTML target {html} of {target} is not registered by \
             {project}"
        );
        let renderer = view.renderer(&html).await?;
        ensure!(
            !renderer.owns_files(),
            "negotiated HTML target {html} of {target} publishes no entry \
             document"
        );
        let destination = view.text(&html, "destination").await?;
        Ok(Some(Self {
            path,
            html: format!("{}/index.html", destination.trim_end_matches('/')),
        }))
    }
}
