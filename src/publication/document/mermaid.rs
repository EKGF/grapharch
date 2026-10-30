//! Bounded native layout of collected Mermaid flowcharts into retained SVG.
use {
    super::Documentor,
    crate::publication::{
        collect::ImageAsset,
        dataset::{Dataset, Stage, identity},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    async_trait::async_trait,
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
    pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd},
    std::collections::BTreeSet,
};

/// Generate retained SVG assets for Mermaid fences in a pinned RDF revision.
pub struct MermaidDocumentor {
    dataset:  Dataset,
    snapshot: NamedNode,
    project:  String,
}
impl MermaidDocumentor {
    /// Consume collected fragments without rereading their original files.
    pub fn new(
        dataset: Dataset,
        snapshot: NamedNode,
        project: &str,
    ) -> Result<Self> {
        NamedNode::new(project)?;
        Ok(Self { dataset, snapshot, project: project.into() })
    }

    async fn asset(
        &self,
        code: String,
        asset: &NamedNode,
        writer: &GraphWriter,
    ) -> Result<()> {
        let processor = identity("mermaid-asset-documentor", &[&self.project]);
        let revision = format!("mermaid-svg-0.3.1-v1:{asset}");
        if self
            .dataset
            .is_current(
                Stage::Document,
                &self.project,
                processor.as_str(),
                asset.as_str(),
                &revision,
            )
            .await?
        {
            return Ok(());
        }
        let mut stream = writer
            .begin_stage(
                &self.project,
                processor.as_str(),
                asset.as_str(),
                &revision,
                Stage::Document,
            )
            .await?;
        let result = async {
            let svg = tokio::task::spawn_blocking(move || -> Result<String> {
                ensure!(
                    !code
                        .lines()
                        .any(|line| line.trim_start().starts_with("%%{")),
                    "Mermaid directives are unsupported"
                );
                let parsed = mermaid_rs_renderer::parse_mermaid_strict(&code)
                    .map_err(|error| {
                    anyhow::anyhow!("invalid Mermaid: {error}")
                })?;
                ensure!(
                    parsed.init_config.is_none(),
                    "Mermaid initialization directives are unsupported"
                );
                ensure!(
                    matches!(
                        parsed.graph.kind,
                        mermaid_rs_renderer::DiagramKind::Flowchart
                    ),
                    "native Mermaid currently supports flowcharts only"
                );
                ensure!(
                    parsed.graph.nodes.len() <= 128 &&
                        parsed.graph.edges.len() <= 256 &&
                        parsed.graph.subgraphs.len() <= 32,
                    "Mermaid diagram exceeds layout budget"
                );
                ensure!(
                    !parsed.graph.nodes.is_empty(),
                    "Mermaid flowchart has no nodes"
                );
                ensure!(
                    parsed.graph.node_links.is_empty(),
                    "interactive Mermaid links are unsupported in retained \
                     diagrams"
                );
                ensure!(
                    parsed.graph.nodes.values().all(|node| {
                        !node.id.contains(['[', ']', '(', ')', '{', '}'])
                    }),
                    "malformed Mermaid node declaration"
                );
                let options = mermaid_rs_renderer::RenderOptions::default();
                let layout = mermaid_rs_renderer::compute_layout(
                    &parsed.graph,
                    &options.theme,
                    &options.layout,
                );
                Ok(mermaid_rs_renderer::render_svg(
                    &layout,
                    &options.theme,
                    &options.layout,
                ))
            })
            .await??;
            ImageAsset {
                project:   &self.project,
                processor: processor.as_str(),
                source:    asset.as_str(),
                id:        asset,
            }
            .write(svg.as_bytes(), "image/svg+xml", &mut stream)
            .await?;

            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(error) = result {
            stream.abort().await?;
            return Err(error);
        }
        stream.complete().await?;
        Ok(())
    }

    async fn fragment(
        &self,
        fragment: &NamedNode,
        body: &str,
        writer: &GraphWriter,
        assets: &mut BTreeSet<NamedNode>,
    ) -> Result<bool> {
        ensure!(
            body.len() <= 1024 * 1024,
            "Mermaid source fragment exceeds 1 MiB"
        );
        if !body.contains("mermaid") {
            return Ok(false);
        }
        let processor = identity("mermaid-documentor", &[&self.project]);
        let mut stream = writer
            .begin_stage(
                &self.project,
                processor.as_str(),
                fragment.as_str(),
                &format!("mermaid-v1:{}", self.snapshot),
                Stage::Document,
            )
            .await?;
        let result = async {
            let mut events = Parser::new_ext(body, crate::publication::markdown::options());
            let mut count = 0;
            while let Some(event) = events.next() {
                if !matches!(event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language))) if language.as_ref() == "mermaid") { continue; }
                count += 1;
                ensure!(count <= 16, "fragment exceeds sixteen Mermaid diagrams");
                let mut code = String::new();
                for event in events.by_ref() {
                    match event {
                        Event::End(TagEnd::CodeBlock) => break,
                        Event::Text(text) => code.push_str(&text),
                        _ => anyhow::bail!("unexpected event in Mermaid fence"),
                    }
                    ensure!(code.len() <= 32 * 1024, "Mermaid diagram exceeds 32 KiB");
                }
                let asset = NamedNode::new(crate::publication::mermaid::id(&code))?;
                if assets.insert(asset.clone()) {
                    ensure!(assets.len() <= 256, "Mermaid publication exceeds 256 assets");
                    self.asset(code, &asset, writer).await?;
                }
                let reference = identity("mermaid-reference", &[fragment.as_str(), asset.as_str()]);
                for quad in [
                    q(fragment, "asset", asset.clone()),
                    q(fragment, "reference", reference.clone()),
                    q(&reference, "type", term("ExternalReference")),
                    q(&reference, "asset", asset.clone()),
                    q(&reference, "url", Literal::new_simple_literal(asset.as_str())),
                    q(&reference, "inputRevision", self.snapshot.clone()),
                    Quad::new(reference, NamedNode::new_unchecked("http://purl.org/dc/terms/identifier"), Literal::new_simple_literal(asset.as_str()), GraphName::DefaultGraph),
                ] { stream.write(quad).await?; }
            }
            Ok::<_, anyhow::Error>(count > 0)
        }.await;
        let present = match result {
            Ok(present) => present,
            Err(error) => {
                stream.abort().await?;
                return Err(
                    error.context(format!("Mermaid fragment {fragment}"))
                );
            },
        };
        stream.complete().await?;
        Ok(present)
    }
}
fn q(subject: &NamedNode, predicate: &str, value: impl Into<Term>) -> Quad {
    Quad::new(
        subject.clone(),
        if predicate == "type" {
            NamedNode::new_unchecked(crate::publication::RDF_TYPE)
        } else {
            term(predicate)
        },
        value,
        GraphName::DefaultGraph,
    )
}
#[async_trait]
impl Documentor for MermaidDocumentor {
    async fn document(&self, writer: &GraphWriter) -> Result<()> {
        let view =
            RevisionView::new(self.dataset.clone(), self.snapshot.clone());
        let pattern = view.pattern(
            &format!(
                "?fragment a {} ; {} \"text/markdown\" ; {} ?body",
                term("Fragment"),
                term("format"),
                term("body")
            ),
            0,
        );
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?fragment ?body WHERE {{ {pattern} \
                 FILTER(CONTAINS(?body, \"mermaid\")) }} ORDER BY ?fragment"
            ))
            .await?;
        // bounded: at most 8192 fragment identities and 256 retained assets.
        let mut fragments = BTreeSet::new();
        let mut assets = BTreeSet::new();
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= 8192,
                "Mermaid documentor exceeds fragment budget"
            );
            let Term::NamedNode(fragment) = &row["fragment"] else {
                anyhow::bail!("Markdown fragment must be an IRI")
            };
            let Term::Literal(body) = &row["body"] else {
                anyhow::bail!("Markdown body must be a literal")
            };
            if self
                .fragment(fragment, body.value(), writer, &mut assets)
                .await?
            {
                fragments.insert(fragment.clone());
            }
        }
        let project = NamedNode::new(&self.project)?;
        let refs = identity("mermaid-documentor", &[&self.project]);
        let images = identity("mermaid-asset-documentor", &[&self.project]);
        let mut previous = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?source ?processor WHERE {{ VALUES \
                 ?processor {{ {refs} {images} }} GRAPH <{}> {{ ?owner {} \
                 ?version . ?version {} ?attempt . ?attempt {} {project} ; {} \
                 ?processor ; {} ?source }} }}",
                crate::publication::dataset::CONTROL,
                term("head"),
                term("data"),
                term("project"),
                term("collector"),
                term("source")
            ))
            .await?;
        let mut owned = 0;
        while let Some(row) = previous.next().await? {
            owned += 1;
            ensure!(owned <= 8448, "Mermaid ownership exceeds budget");
            let (Term::Literal(source), Term::NamedNode(processor)) =
                (&row["source"], &row["processor"])
            else {
                anyhow::bail!("invalid Mermaid ownership")
            };
            let source_id = NamedNode::new(source.value())?;
            let present = if processor == &images {
                assets.contains(&source_id)
            } else {
                fragments.contains(&source_id)
            };
            if !present {
                writer
                    .remove(&self.project, processor.as_str(), source.value())
                    .await?;
            }
        }
        Ok(())
    }
}
