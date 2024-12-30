use {
    super::super::{FileSourceCreator, FileSourceVariant},
    crate::{
        source::FileSource,
        util::{FileType, FileTypeSlice, contents_of_local_file},
    },
    async_trait::async_trait,
    std::path::{Path, PathBuf},
};

/// A local directory source for scanning files.
#[derive(Debug, Clone)]
pub struct FileSystemSourceImpl {
    root_path: PathBuf,
}

impl FileSourceCreator for FileSystemSourceImpl {
    fn new(
        root_path: Option<&Path>,
        _endpoint_url: Option<&str>,
    ) -> anyhow::Result<Self> {
        if let Some(ref rp) = root_path.map(|p| p.to_path_buf()) {
            if !rp.exists() {
                return Err(anyhow::anyhow!(
                    "Directory does not exist: {}",
                    rp.display()
                ));
            }
            Ok(Self { root_path: rp.canonicalize()? })
        } else {
            Err(anyhow::anyhow!("Root path is required"))
        }
    }
}

#[async_trait]
impl FileSource for FileSystemSourceImpl {
    /// Scan the given root directory for any files with given file
    /// types. We're using the ignore package since that has
    /// excellent support for all kinds of "ignore file formats"
    /// such as `.gitignore` which will be respected.
    async fn scan<'a>(
        &self,
        types: FileTypeSlice<'a>,
    ) -> anyhow::Result<Vec<PathBuf>> {
        Self::scan_files(types, self.root_path.clone()).await
    }

    fn root_path(&self) -> Option<&Path> { Some(&self.root_path) }

    fn url(&self) -> Option<&str> { None }

    fn variant(&self) -> FileSourceVariant {
        FileSourceVariant::FileSystem
    }

    async fn content_of(
        &self,
        file_path: &Path,
    ) -> anyhow::Result<String> {
        contents_of_local_file(file_path).await
    }
}

impl FileSystemSourceImpl {
    pub async fn scan_files<'a>(
        types: FileTypeSlice<'a>,
        root_directory: PathBuf,
    ) -> anyhow::Result<Vec<PathBuf>> {
        let types_vec = FileType::from_slice_to_cloned_vec(types);
        let ignore_types = Self::ignore_crate_types(types)?;
        let files: anyhow::Result<Vec<PathBuf>> =
            tokio::task::spawn_blocking(move || {
                // The ignore create is not thread safe, so we need to
                // spawn a new thread to do the scanning.
                // However, the ignore is very good at handling files
                // like .gitignore, skipping a lot of
                // files that are definitely not interesting to us.
                let walker = ignore::WalkBuilder::new(root_directory)
                    .hidden(true)
                    .ignore(true)
                    .parents(false)
                    .git_global(true)
                    .git_ignore(true)
                    .git_exclude(true)
                    .types(ignore_types)
                    .build();
                let types_ref =
                    FileType::create_vec_of_references(&types_vec);
                let mut files = Vec::new();
                for result in walker {
                    let entry = result.map_err(|e| {
                        anyhow::anyhow!(
                            "Error during directory traversal: {}",
                            e
                        )
                    })?;
                    let path = entry.path();
                    if FileType::is_matching_file_type(
                        path,
                        &types_ref[..],
                    ) {
                        files.push(path.to_path_buf());
                    }
                }
                Ok(files)
            })
            .await?;
        let mut files = files?;
        files.sort();
        Ok(files)
    }

    /// Creates a Types object (which is a thing in the ignores crate)
    /// from the given file types.
    pub fn ignore_crate_types<'a>(
        file_types: &'a [&'a FileType],
    ) -> anyhow::Result<ignore::types::Types> {
        let mut builder = ignore::types::TypesBuilder::new();
        for file_type in file_types {
            file_type.to_ignore_crate_type(&mut builder)?;
        }
        Ok(builder.build().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::source::{
            FileSource,
            FileSourceImplementor,
            FileSourceVariant,
        },
    };

    #[tokio::test]
    async fn test_scan() -> anyhow::Result<()> {
        let source = FileSourceImplementor::new(
            FileSourceVariant::FileSystem,
            Some(&Path::new(".")),
            None,
        )?;
        let files = source
            .scan(&[&FileType::Markdown].to_vec())
            .await
            .unwrap();
        println!("{:?}", files);
        Ok(())
    }
}
