use std::path::{Path, PathBuf};

/// Derive the top-level but most specific directory from the
/// given set of file names.
pub fn derive_root_directory<P: AsRef<Path>>(files: &[P]) -> Option<PathBuf> {
    // If no files, return None.
    let first = files.first()?;

    // Start with the components of the first file’s parent directory.
    let mut common_components: Vec<_> =
        first.as_ref().parent()?.components().collect();

    // For each subsequent file, compare components.
    for file in &files[1..] {
        let parent_components: Vec<_> =
            file.as_ref().parent()?.components().collect();

        // Find how many leading components match.
        let mut i = 0;
        while i < common_components.len() &&
            i < parent_components.len() &&
            common_components[i] == parent_components[i]
        {
            i += 1;
        }

        // Truncate `common_components` to only the shared prefix.
        common_components.truncate(i);

        // If there's no overlap at all, there's no common directory.
        if common_components.is_empty() {
            return None;
        }
    }

    // Rebuild the path from the shared components.
    let mut root_dir = PathBuf::new();
    for comp in common_components {
        root_dir.push(comp.as_os_str());
    }

    Some(root_dir)
}

pub fn relative_path<P1: AsRef<Path>, P2: AsRef<Path>>(
    path: P1,
    root_path: P2,
) -> PathBuf {
    path.as_ref()
        .strip_prefix(root_path.as_ref())
        .unwrap_or_else(|_| path.as_ref())
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::path::{Path, PathBuf},
    };

    #[test]
    fn test_derive_root_directory_single_file() {
        let file_names = vec![PathBuf::from("path/to/file.txt")];
        let root_dir = derive_root_directory(&file_names);
        let expected_root = Path::new("path/to");

        assert!(root_dir.is_some());
        assert_eq!(root_dir.unwrap().as_path(), expected_root)
    }

    #[test]
    fn test_derive_root_directory_multiple_files_same_dir() {
        let file_names = vec![
            PathBuf::from("path/to/file1.txt"),
            PathBuf::from("path/to/file2.txt"),
        ];
        let root_dir = derive_root_directory(&file_names);
        let expected_root = Path::new("path/to");

        assert!(root_dir.is_some());
        assert_eq!(root_dir.unwrap().as_path(), expected_root)
    }

    #[test]
    fn test_derive_root_directory_multiple_files_different_dirs() {
        let file_names = vec![
            PathBuf::from("path/to/dir1/file1.txt"),
            PathBuf::from("path/to/dir2/file2.txt"),
        ];
        let root_dir = derive_root_directory(&file_names);
        let expected_root = Path::new("path/to");

        assert!(root_dir.is_some());
        assert_eq!(root_dir.unwrap().as_path(), expected_root)
    }

    #[test]
    fn test_derive_root_directory_nested_dirs() {
        let file_names = vec![
            PathBuf::from("path/to/dir1/subdir1/file1.txt"),
            PathBuf::from("path/to/dir2/subdir2/file2.txt"),
        ];
        let root_dir = derive_root_directory(&file_names);
        let expected_root = Path::new("path/to");

        assert!(root_dir.is_some());
        assert_eq!(root_dir.unwrap().as_path(), expected_root)
    }

    #[test]
    fn test_derive_root_directory_empty_input() {
        let file_names: Vec<PathBuf> = vec![];

        assert_eq!(derive_root_directory(&file_names), None);
    }
}
