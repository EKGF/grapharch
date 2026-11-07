//! Restore one validated image from its pinned, bounded KG chunks.
use {
    super::model::Plan,
    crate::publication::term,
    anyhow::{Result, ensure},
    base64::{Engine, engine::general_purpose::STANDARD},
    oxrdf::{NamedNode, Term},
    sha2::{Digest, Sha256},
};
pub(super) async fn read(
    plan: &Plan,
    id: &NamedNode,
    mime: &str,
) -> Result<Vec<u8>> {
    let size: usize = plan.source.text(id, "contentSize").await?.parse()?;
    ensure!(
        size > 0 && size <= crate::publication::image_assets::MAX_IMAGE_BYTES,
        "collected image exceeds byte budget"
    );
    let digest = plan.source.text(id, "contentDigest").await?;
    let pattern = plan.source.pattern(
        &format!(
            "{id} {} ?chunk . ?chunk {} ?order ; {} ?data",
            term("chunk"),
            term("order"),
            term("base64")
        ),
        204,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?order ?data WHERE {{ {pattern} }} ORDER BY \
             ?order"
        ))
        .await?;
    let mut bytes = Vec::with_capacity(size);
    let mut order = 0;
    while let Some(row) = rows.next().await? {
        ensure!(
            order < crate::publication::image_assets::MAX_IMAGE_CHUNKS,
            "image chunk count exceeds budget"
        );
        let (Term::Literal(position), Term::Literal(data)) =
            (&row["order"], &row["data"])
        else {
            anyhow::bail!("image chunks require literal order and data")
        };
        ensure!(
            position.value().parse::<usize>()? == order,
            "image chunks must have consecutive orders"
        );
        ensure!(
            data.value().len() <= 64 * 1024,
            "image chunk exceeds byte budget"
        );
        let chunk = STANDARD.decode(data.value())?;
        ensure!(
            bytes.len() + chunk.len() <= size,
            "image exceeds declared size"
        );
        bytes.extend_from_slice(&chunk);
        order += 1;
    }
    ensure!(
        bytes.len() == size && hex::encode(Sha256::digest(&bytes)) == digest,
        "image asset size or digest mismatch"
    );
    crate::publication::image_assets::validate(&bytes, mime)?;
    Ok(bytes)
}
