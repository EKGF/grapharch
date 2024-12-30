use {
    super::super::{
        FileSource,
        FileSourceCreator,
        FileSourceVariant,
    },
    crate::{
        source::FileSystemSourceImpl,
        util::{FileTypeSlice, contents_of_local_file},
    },
    anyhow::Context,
    async_trait::async_trait,
    git2::Repository,
    std::{
        env,
        path::{Path, PathBuf},
        vec::Vec,
    },
    tokio::fs,
};

/// A source for reading files from a Git repository asynchronously.
#[derive(Debug, Clone)]
pub struct GitRepositorySourceImpl {
    repo_url: String,
    temp_dir: PathBuf,
}

impl FileSourceCreator for GitRepositorySourceImpl {
    fn new(
        _root_path: Option<&Path>,
        endpoint_url: Option<&str>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            repo_url: endpoint_url.unwrap().to_string(),
            temp_dir: env::temp_dir().join("file_repo"),
        })
    }
}

#[async_trait]
impl FileSource for GitRepositorySourceImpl {
    async fn scan<'a>(
        &self,
        types: FileTypeSlice<'a>,
    ) -> anyhow::Result<Vec<PathBuf>> {
        let repo_url = self.repo_url.clone();
        let temp_dir = self.temp_dir.clone();

        if Path::new(&temp_dir).exists() {
            fs::remove_dir_all(&temp_dir)
                .await
                .context("Failed to clear temp directory")?;
        }
        fs::create_dir_all(&temp_dir)
            .await
            .context("Failed to create temp directory")?;

        tokio::task::spawn_blocking(move || {
            Repository::clone_recurse(&repo_url, &temp_dir)
                .context("Failed to clone repository")
        })
        .await??;

        let files = FileSystemSourceImpl::scan_files(
            types,
            self.temp_dir.clone(),
        )
        .await?;

        tracing::info!(
            "Found {} files in git repository {}",
            files.len(),
            self.repo_url
        );

        Ok(files)
    }

    fn root_path(&self) -> Option<&Path> { Some(&self.temp_dir) }

    fn url(&self) -> Option<&str> { Some(&self.repo_url) }

    fn variant(&self) -> FileSourceVariant {
        FileSourceVariant::GitRepository
    }

    async fn content_of(
        &self,
        file_path: &Path,
    ) -> anyhow::Result<String> {
        contents_of_local_file(file_path).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        source::{
            FileSource,
            FileSourceImplementor,
            FileSourceVariant,
        },
        util::{FileType, derive_root_directory},
    };

    #[tokio::test]
    async fn test_scan() -> anyhow::Result<()> {
        let source = FileSourceImplementor::new(
            FileSourceVariant::GitRepository,
            None,
            Some("https://github.com/EKGF/grapharch.git"),
        )?;
        let files =
            source.scan(&[&FileType::Markdown]).await.unwrap();
        assert!(files.len() > 0);
        for file in &files {
            println!("{}", file.display());
        }

        let root_dir = derive_root_directory(&files);
        assert!(&root_dir.is_some());
        if let Some(root_dir) = &root_dir {
            assert!(files.contains(&root_dir.join("README.md")));
            assert!(
                files.contains(&root_dir.join("CONTRIBUTING.md"))
            );
        }

        Ok(())
    }
}
