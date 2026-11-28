//! Zero-configuration collection of a tree materialized from a pinned source.
use {
    super::{
        super::{collect::Collector, dataset::identity, plan::ResolvedPlan},
        discover::Inventory,
        plain,
    },
    crate::publication::{
        dataset::Dataset,
        repository::{ForgeAccess, RepositoryDiscoverer, RepositoryRecord},
    },
    anyhow::{Result, ensure},
    std::{path::Path, sync::Arc},
};

/// Collect a materialized tree with the zero-configuration defaults.
///
/// The project identity derives from `source`, the stable identity of the
/// pinned origin, so it does not depend on where the tree was materialized.
/// A tree carrying GraphArch or MkDocs configuration must use the runner that
/// honours that configuration instead.
pub(crate) async fn collect(
    root: &Path,
    source: &str,
    extra: Vec<Arc<dyn Collector>>,
) -> Result<(Dataset, ResolvedPlan)> {
    let inventory = Inventory::read(
        root,
        &root.join(".grapharch-output"),
        crate::publication::source_walk::SourceBudget::DEFAULT,
    )
    .await?;
    ensure!(
        inventory.definitions.is_none() && inventory.mkdocs.is_none(),
        "a zero-configuration tree cannot contain GraphArch or MkDocs \
         configuration; use the exact-revision or MkDocs runner"
    );
    // A pinned build never reads live forge data: Git and manifests only.
    let facts = RepositoryDiscoverer::new(ForgeAccess::Offline)
        .discover(root)
        .await;
    let project = identity("quick-project", &[source]);
    let repository = super::Repository {
        name:    facts.name(),
        package: facts.has_package(),
        web:     facts.forge.as_ref().map(|forge| forge.web_url()),
        policy:  crate::publication::reference::LinkPolicy::Lenient,
    };
    let mut extra = extra;
    extra.push(Arc::new(RepositoryRecord::new(
        project.clone(),
        source.to_owned(),
        facts,
    )));
    plain(&inventory, project, extra, &repository).await
}
