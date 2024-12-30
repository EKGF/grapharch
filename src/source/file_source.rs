use {
    super::r#impl::{
        FileSystemSourceImpl,
        GitRepositorySourceImpl,
        S3BucketSourceImpl,
    },
    crate::util::{FileTypeSlice, relative_path},
    async_trait::async_trait,
    std::{
        path::{Path, PathBuf},
        vec::Vec,
    },
};

/// A trait for sources that provide files of various types
/// asynchronously.
#[async_trait]
pub trait FileSource: Sync {
    /// Asynchronously scans the source for files with the specified
    /// types.
    async fn scan<'a>(
        &self,
        types: FileTypeSlice<'a>,
    ) -> anyhow::Result<Vec<PathBuf>>;

    /// Returns the root path of the file source.
    fn root_path(&self) -> Option<&Path>;

    /// Returns the URL of the Git repository or S3 bucket.
    fn url(&self) -> Option<&str>;

    /// Returns the variant of the file source.
    fn variant(&self) -> FileSourceVariant;

    /// Returns the content of the file at the given path.
    async fn content_of(&self, file_path: &Path) -> anyhow::Result<String>;
}

pub trait FileSourceCreator: Sized {
    /// Creates a new `FileSource`.
    ///
    /// # Arguments
    ///
    /// * `root_path` - The root path of the file source if relevant.
    /// * `endpoint_url` - The URL of the Git repository or S3 bucket if
    ///   relevant.
    fn new(
        root_path: Option<&Path>,
        endpoint_url: Option<&str>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSourceVariant {
    FileSystem,
    GitRepository,
    S3Bucket,
}

#[derive(Debug, Clone)]
pub enum FileSourceImplementor {
    LocalDirectorySource(FileSystemSourceImpl),
    GitRepositorySource(GitRepositorySourceImpl),
    S3BucketSource(S3BucketSourceImpl),
}

impl FileSourceImplementor {
    pub fn new(
        variant: FileSourceVariant,
        root_path: Option<&Path>,
        repo_url: Option<&str>,
    ) -> anyhow::Result<Self> {
        match variant {
            FileSourceVariant::FileSystem => {
                Ok(FileSourceImplementor::LocalDirectorySource(
                    FileSystemSourceImpl::new(root_path, repo_url)?,
                ))
            },
            FileSourceVariant::GitRepository => {
                Ok(FileSourceImplementor::GitRepositorySource(
                    GitRepositorySourceImpl::new(root_path, repo_url)?,
                ))
            },
            FileSourceVariant::S3Bucket => {
                Ok(FileSourceImplementor::S3BucketSource(
                    S3BucketSourceImpl::new(root_path, repo_url)?,
                ))
            },
        }
    }
}

#[async_trait]
impl FileSource for FileSourceImplementor {
    async fn scan<'a>(
        &self,
        types: FileTypeSlice<'a>,
    ) -> anyhow::Result<Vec<PathBuf>> {
        tracing::info!(
            "Scanning for files with types: {:}",
            types
                .iter()
                .map(|f| f.as_ref())
                .collect::<Vec<&str>>()
                .join(", ")
        );
        let file_names = match self {
            FileSourceImplementor::LocalDirectorySource(source) => {
                source.scan(types).await
            },
            FileSourceImplementor::GitRepositorySource(source) => {
                source.scan(types).await
            },
            FileSourceImplementor::S3BucketSource(source) => {
                source.scan(types).await
            },
        }?;
        tracing::info!("Found {} files", file_names.len());
        let root_path = self.root_path().unwrap();
        for file_name in &file_names {
            tracing::info!(
                " - {}",
                relative_path(file_name, root_path).display()
            );
        }

        Ok(file_names)
    }

    fn root_path(&self) -> Option<&Path> {
        match self {
            FileSourceImplementor::LocalDirectorySource(source) => {
                source.root_path()
            },
            FileSourceImplementor::GitRepositorySource(source) => {
                source.root_path()
            },
            FileSourceImplementor::S3BucketSource(source) => source.root_path(),
        }
    }

    fn url(&self) -> Option<&str> {
        match self {
            FileSourceImplementor::GitRepositorySource(source) => source.url(),
            FileSourceImplementor::S3BucketSource(source) => source.url(),
            _ => None,
        }
    }

    fn variant(&self) -> FileSourceVariant {
        match self {
            FileSourceImplementor::LocalDirectorySource(_) => {
                FileSourceVariant::FileSystem
            },
            FileSourceImplementor::GitRepositorySource(_) => {
                FileSourceVariant::GitRepository
            },
            FileSourceImplementor::S3BucketSource(_) => {
                FileSourceVariant::S3Bucket
            },
        }
    }

    async fn content_of(&self, file_path: &Path) -> anyhow::Result<String> {
        match self {
            FileSourceImplementor::LocalDirectorySource(source) => {
                source.content_of(file_path).await
            },
            FileSourceImplementor::GitRepositorySource(source) => {
                source.content_of(file_path).await
            },
            FileSourceImplementor::S3BucketSource(source) => {
                source.content_of(file_path).await
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            source::{FileSource, FileSourceImplementor, FileSourceVariant},
            util::FileType,
        },
        std::env::current_dir,
    };

    #[tokio::test]
    async fn test_scan() -> anyhow::Result<()> {
        let source = FileSourceImplementor::new(
            FileSourceVariant::FileSystem,
            Some(&Path::new(".")),
            None,
        )?;
        let files = source.scan(&[&FileType::Markdown]).await?;
        assert!(files.len() > 0);
        for file in &files {
            println!("{}", file.display());
        }
        let our_readme = current_dir()?.join("README.md");
        assert!(&files.contains(&our_readme));
        Ok(())
    }
}
