//! Zero-configuration collection of a tree materialized from a pinned source.
use {
    super::{
        super::{collect::Collector, dataset::identity, plan::ResolvedPlan},
        discover::Inventory,
        plain,
    },
    crate::publication::dataset::Dataset,
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
    let inventory =
        Inventory::read(root, &root.join(".grapharch-output")).await?;
    ensure!(
        inventory.definitions.is_none() && inventory.mkdocs.is_none(),
        "a zero-configuration tree cannot contain GraphArch or MkDocs \
         configuration; use the exact-revision or MkDocs runner"
    );
    plain(
        &inventory,
        identity("quick-project", &[source]),
        extra,
    )
    .await
}
