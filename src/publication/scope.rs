//! Subject selection stored in graph-backed working sets rather than Rust
//! collections.

use {
    super::{
        RDF_TYPE,
        dataset::{Dataset, MAX_CHUNK_QUADS, Stage, identity},
        term,
        writer::GraphWriter,
    },
    anyhow::{Result, ensure},
    futures::future::BoxFuture,
    oxrdf::{NamedNode, Quad, Term},
    uuid::Uuid,
};

/// A resolved scope and its immutable contribution view.
#[derive(Clone, Debug)]
pub struct ScopePlan {
    /// Stable plan identity.
    pub id:       NamedNode,
    /// Snapshot containing exactly this completed plan contribution.
    pub snapshot: NamedNode,
}

/// Resolve reusable scopes against exactly one pinned input snapshot.
pub struct ScopeResolver {
    dataset:  Dataset,
    snapshot: NamedNode,
}

impl ScopeResolver {
    /// Bind the resolver to an immutable input revision.
    pub fn new(dataset: Dataset, snapshot: NamedNode) -> Self {
        Self { dataset, snapshot }
    }

    /// Resolve explicit membership, composed scopes, exclusions, and bounded
    /// traversal. Exclusions remove members and prevent this scope's
    /// traversal through them.
    pub async fn resolve(
        &self,
        project: &str,
        scope: &NamedNode,
        writer: &GraphWriter,
    ) -> Result<ScopePlan> {
        let registry = NamedNode::new(format!(
            "urn:grapharch:scope-work:{}",
            Uuid::new_v4()
        ))?;
        let result =
            self.resolve_inner(project, scope, writer, &registry).await;
        let cleanup = format!(
            "DELETE {{ GRAPH ?graph {{ ?s ?p ?o }} }} WHERE {{ GRAPH \
             {registry} {{ ?graph {} ?scope }} GRAPH ?graph {{ ?s ?p ?o }} \
             }}; DROP SILENT GRAPH {registry}",
            term("scope")
        );
        self.update(cleanup).await?;
        result
    }

    async fn resolve_inner(
        &self,
        project: &str,
        scope: &NamedNode,
        writer: &GraphWriter,
        registry: &NamedNode,
    ) -> Result<ScopePlan> {
        let graph = self
            .build_scope(scope.clone(), registry.clone(), Vec::new())
            .await?;
        let plan = identity("scope-plan", &[
            project,
            scope.as_str(),
            self.snapshot.as_str(),
        ]);
        let mut stream = writer
            .begin_stage(
                project,
                "urn:grapharch:scope-resolver:v1",
                scope.as_str(),
                self.snapshot.as_str(),
                Stage::Plan,
            )
            .await?;
        // bounded: one output chunk, capped at MAX_CHUNK_QUADS.
        let mut chunk = vec![
            Quad::new(
                plan.clone(),
                NamedNode::new_unchecked(RDF_TYPE),
                term("ScopePlan"),
                plan.clone(),
            ),
            Quad::new(
                plan.clone(),
                term("scope"),
                scope.clone(),
                plan.clone(),
            ),
            Quad::new(
                plan.clone(),
                term("inputRevision"),
                self.snapshot.clone(),
                plan.clone(),
            ),
        ];
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?member WHERE {{ GRAPH {graph} {{ {scope} {} \
                 ?member }} }}",
                term("member")
            ))
            .await?;
        while let Some(row) = rows.next().await? {
            let Some(Term::NamedNode(member)) = row.get("member") else {
                anyhow::bail!("scope members require stable IRIs")
            };
            chunk.push(Quad::new(
                plan.clone(),
                term("member"),
                member.clone(),
                plan.clone(),
            ));
            if chunk.len() == MAX_CHUNK_QUADS {
                stream.push(std::mem::take(&mut chunk)).await?;
            }
        }
        if !chunk.is_empty() {
            stream.push(chunk).await?;
        }
        let receipt = stream.commit().await?;
        let snapshot = self.dataset.pin(&receipt).await?;
        Ok(ScopePlan { id: plan, snapshot })
    }

    fn build_scope(
        &self,
        scope: NamedNode,
        registry: NamedNode,
        mut path: Vec<NamedNode>,
    ) -> BoxFuture<'_, Result<NamedNode>> {
        Box::pin(async move {
            // bounded: composition nesting has an explicit 64-level execution
            // limit.
            ensure!(
                path.len() < 64,
                "scope composition exceeds 64 levels"
            );
            ensure!(
                !path.contains(&scope),
                "cyclic scope composition at {scope}"
            );
            path.push(scope.clone());
            let kind = self.pattern(
                &format!("{scope} <{RDF_TYPE}> {}", term("Scope")),
                0,
            );
            let mut defined = self
                .dataset
                .select(format!(
                    "SELECT ?_owner0 WHERE {{ {kind} }} LIMIT 1"
                ))
                .await?;
            ensure!(
                defined.next().await?.is_some(),
                "undefined scope {scope}"
            );
            drop(defined);
            let graph = identity("scope-working-set", &[
                registry.as_str(),
                scope.as_str(),
            ]);
            let include = self
                .pattern(&format!("{scope} {} ?member", term("include")), 0);
            self.update(format!(
                "INSERT DATA {{ GRAPH {registry} {{ {graph} {} {scope} }} }}; \
                 INSERT {{ GRAPH {graph} {{ {scope} {} ?member }} }} WHERE {{ \
                 {include} }}",
                term("scope"),
                term("member")
            ))
            .await?;
            let composed =
                self.pattern(&format!("{scope} {} ?child", term("compose")), 0);
            let mut children = self
                .dataset
                .select(format!(
                    "SELECT DISTINCT ?child WHERE {{ {composed} }}"
                ))
                .await?;
            while let Some(row) = children.next().await? {
                let Some(Term::NamedNode(child)) = row.get("child") else {
                    anyhow::bail!("composed scope must be an IRI")
                };
                let child_graph = self
                    .build_scope(child.clone(), registry.clone(), path.clone())
                    .await?;
                self.update(format!(
                    "INSERT {{ GRAPH {graph} {{ {scope} {} ?member }} }} \
                     WHERE {{ GRAPH {child_graph} {{ {child} {} ?member }} }}",
                    term("member"),
                    term("member")
                ))
                .await?;
            }
            let exclude = self
                .pattern(&format!("{scope} {} ?member", term("exclude")), 0);
            self.update(format!(
                "DELETE {{ GRAPH {graph} {{ {scope} {} ?member }} }} WHERE {{ \
                 GRAPH {graph} {{ {scope} {} ?member }} {exclude} }}",
                term("member"),
                term("member")
            ))
            .await?;
            let depth = self.depth(&scope).await?;
            let traversal = self.pattern(
                &format!("{scope} {} ?predicate", term("traverse")),
                0,
            );
            let edge = self.pattern("?parent ?predicate ?member", 1);
            let excluded = self
                .pattern(&format!("{scope} {} ?member", term("exclude")), 2);
            let next = format!(
                "GRAPH {graph} {{ {scope} {} ?parent }} {traversal} {edge} \
                 FILTER NOT EXISTS {{ GRAPH {graph} {{ {scope} {} ?member }} \
                 }} FILTER NOT EXISTS {{ {excluded} }}",
                term("member"),
                term("member")
            );
            for _ in 0..depth {
                let mut pending = self
                    .dataset
                    .select(format!(
                        "SELECT ?member WHERE {{ {next} }} LIMIT 1"
                    ))
                    .await?;
                if pending.next().await?.is_none() {
                    break;
                }
                drop(pending);
                self.update(format!(
                    "INSERT {{ GRAPH {graph} {{ {scope} {} ?member }} }} \
                     WHERE {{ {next} }}",
                    term("member")
                ))
                .await?;
            }
            Ok(graph)
        })
    }

    async fn depth(&self, scope: &NamedNode) -> Result<u32> {
        let pattern =
            self.pattern(&format!("{scope} {} ?depth", term("maxDepth")), 0);
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT DISTINCT ?depth WHERE {{ {pattern} }}"
            ))
            .await?;
        let mut depth = None;
        while let Some(row) = rows.next().await? {
            ensure!(
                depth.is_none(),
                "scope has conflicting maxDepth values"
            );
            depth = Some(super::validation::unsigned(
                &row["depth"],
                "scope maxDepth",
            )?);
        }
        let traversal = self.pattern(
            &format!("{scope} {} ?predicate", term("traverse")),
            0,
        );
        let mut rows = self
            .dataset
            .select(format!(
                "SELECT ?predicate WHERE {{ {traversal} }} LIMIT 1"
            ))
            .await?;
        ensure!(
            depth.is_some() || rows.next().await?.is_none(),
            "scope traversal requires explicit maxDepth"
        );
        Ok(depth.unwrap_or(0))
    }

    fn pattern(&self, pattern: &str, index: u16) -> String {
        Dataset::logical_pattern(&self.snapshot, pattern, index)
    }

    async fn update(&self, query: String) -> Result<()> {
        self.dataset
            .run(move |store| {
                store.update(query.as_str())?;
                Ok(())
            })
            .await
    }
}
