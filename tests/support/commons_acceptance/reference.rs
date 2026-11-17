//! The formal OMG Commons 1.3 specification that the GraphArch edition is
//! accepted against, fetched from OMG and pinned by content hash.
use {
    anyhow::{Context, Result, ensure},
    sha2::{Digest, Sha256},
    std::path::{Path, PathBuf},
};

/// OMG's formal/26-06-05 PDF.
pub struct OmgReference;

impl OmgReference {
    /// OMG's file is 5.6 MiB; anything far larger is not the specification.
    const BYTE_LIMIT: usize = 32 * 1024 * 1024;
    /// SHA-256 of the PDF retrieved on 2026-11-11.
    const SHA256: &str =
        "ec807419ea2163d259371f931c06eac3413afb562fadb47017941970658171df";
    const URL: &str = "https://www.omg.org/spec/Commons/1.3/PDF";

    /// The verified PDF bytes, from `cache` when a verified copy is there.
    pub async fn bytes(cache: &Path) -> Result<Vec<u8>> {
        let path = Self::cached(cache);
        if let Ok(bytes) = tokio::fs::read(&path).await &&
            Self::verified(&bytes)
        {
            return Ok(bytes);
        }
        let bytes = Self::download().await?;
        ensure!(
            Self::verified(&bytes),
            "{} no longer serves formal/26-06-05 (SHA-256 {}); the acceptance \
             reference must be re-pinned deliberately",
            Self::URL,
            hex::encode(Sha256::digest(&bytes))
        );
        tokio::fs::create_dir_all(cache).await?;
        tokio::fs::write(&path, &bytes).await?;
        Ok(bytes)
    }

    fn cached(cache: &Path) -> PathBuf {
        cache.join("omg-commons-formal-26-06-05.pdf")
    }

    fn verified(bytes: &[u8]) -> bool {
        hex::encode(Sha256::digest(bytes)) == Self::SHA256
    }

    async fn download() -> Result<Vec<u8>> {
        let response = reqwest::Client::new()
            .get(Self::URL)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .with_context(|| format!("cannot fetch {}", Self::URL))?;
        ensure!(
            response
                .content_length()
                .is_none_or(|length| length <= Self::BYTE_LIMIT as u64),
            "{} exceeds the reference size limit",
            Self::URL
        );
        let bytes = response.bytes().await?;
        ensure!(
            bytes.len() <= Self::BYTE_LIMIT,
            "{} exceeds the reference size limit",
            Self::URL
        );
        Ok(bytes.to_vec())
    }
}
