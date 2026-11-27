//! The part of a repository web URL after the repository itself, as code
//! hosts such as GitHub lay it out.
/// Where inside a repository a link points.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RepositoryPath<'a> {
    /// The repository itself.
    Root,
    /// One file at a reference: `blob/<reference>/<path>`.
    File { reference: &'a str, path: &'a str },
    /// Anything else: directories, issues, releases and other host pages.
    Other,
}

impl<'a> RepositoryPath<'a> {
    /// Parse the URL path following the repository, without a leading slash.
    pub fn parse(rest: &'a str) -> Self {
        if rest.is_empty() {
            return Self::Root;
        }
        let Some(file) = rest.strip_prefix("blob/") else {
            return Self::Other;
        };
        match file.split_once('/') {
            Some((reference, path))
                if !reference.is_empty() &&
                    !path.is_empty() &&
                    !path.ends_with('/') &&
                    !path
                        .split('/')
                        .any(|part| matches!(part, "" | "." | "..")) =>
            {
                Self::File { reference, path }
            },
            _ => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RepositoryPath;

    #[test]
    fn repository_paths_distinguish_files_from_other_pages() {
        assert_eq!(RepositoryPath::parse(""), RepositoryPath::Root);
        assert_eq!(
            RepositoryPath::parse("blob/main/docs/guide.md"),
            RepositoryPath::File {
                reference: "main",
                path:      "docs/guide.md",
            }
        );
        for other in [
            "tree/main/docs",
            "issues/4",
            "blob/main/",
            "blob/main",
            "blob/main/docs/../x.md",
            "releases",
        ] {
            assert_eq!(
                RepositoryPath::parse(other),
                RepositoryPath::Other,
                "{other}"
            );
        }
    }
}
