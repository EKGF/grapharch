//! Restore one collected publication artifact, checking its size and digest.
use {
    super::super::model::Plan,
    anyhow::{Result, ensure},
    base64::{Engine, engine::general_purpose::STANDARD},
    oxrdf::{NamedNode, Term},
    sha2::{Digest, Sha256},
    std::path::Path,
    tokio::io::AsyncWriteExt,
};

pub(super) async fn write(
    plan: &Plan,
    id: &NamedNode,
    output: &Path,
) -> Result<()> {
    let expected_size: u64 =
        plan.source.text(id, "contentSize").await?.parse()?;
    ensure!(
        expected_size <=
            crate::publication::collect::published_artifact::MAX_ARTIFACT_BYTES,
        "collected artifact exceeds byte budget"
    );
    let expected_digest = plan.source.text(id, "contentDigest").await?;
    let pattern = plan.source.pattern(
        &format!(
            "{id} {} ?chunk . ?chunk {} ?order ; {} ?data",
            crate::publication::term("chunk"),
            crate::publication::term("order"),
            crate::publication::term("base64")
        ),
        0,
    );
    let mut rows = plan
        .dataset
        .select(format!(
            "SELECT DISTINCT ?order ?data WHERE {{ {pattern} }} ORDER BY \
             ?order"
        ))
        .await?;
    let mut file = tokio::fs::File::create(output).await?;
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut order = 0u64;
    while let Some(row) = rows.next().await? {
        let Term::Literal(position) = &row["order"] else {
            anyhow::bail!("artifact chunk order must be a literal")
        };
        ensure!(
            position.value().parse::<u64>()? == order,
            "artifact chunks must have consecutive orders"
        );
        let Term::Literal(data) = &row["data"] else {
            anyhow::bail!("artifact data must be a literal")
        };
        ensure!(
            data.value().len() <= 64 * 1024,
            "artifact chunk exceeds byte budget"
        );
        let data = STANDARD.decode(data.value())?;
        bytes += data.len() as u64;
        ensure!(
            bytes <= expected_size,
            "artifact exceeds declared size"
        );
        hash.update(&data);
        file.write_all(&data).await?;
        order += 1;
    }
    ensure!(
        bytes == expected_size &&
            hex::encode(hash.finalize()) == expected_digest,
        "artifact size or digest mismatch"
    );
    file.flush().await?;
    Ok(())
}
