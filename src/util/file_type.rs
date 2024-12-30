use {
    oxrdfio::RdfFormat,
    std::{collections::HashMap, ffi::OsStr, path::Path, sync::LazyLock},
};

pub type FileTypeSlice<'a> = &'a [&'a FileType];
pub type FileTypeSliceStatic = FileTypeSlice<'static>;

const JEKYLL_CONFIG_FILE_NAME: &str = "_config.yml";

/// Enum representing various file types.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum FileType {
    Markdown,
    NTriples,
    RdfXml,
    JSONLD,
    Turtle,
    N3,
    NQuads,
    TriG,
    TOML,
    YAML,
    JSON,
    JekyllConfig,
}

impl FileType {
    /// Returns the name of the file type.
    pub fn name(&self) -> &'static str {
        match self {
            FileType::Markdown => "Markdown",
            FileType::TOML => "TOML",
            FileType::YAML => "YAML",
            FileType::JSON => "JSON",
            FileType::JSONLD => "JSON-LD",
            FileType::N3 |
            FileType::NQuads |
            FileType::NTriples |
            FileType::RdfXml |
            FileType::TriG |
            FileType::Turtle => {
                self.oxi_graph_rdf_format()
                    .map(|f| f.name())
                    .unwrap_or("Unknown")
            },
            FileType::JekyllConfig => "Jekyll Configuration",
        }
    }

    /// Returns the file extension associated with the file type.
    pub fn extension(&self) -> Option<&'static OsStr> {
        match self {
            FileType::Markdown => Some(OsStr::new("md")),
            FileType::TOML => Some(OsStr::new("toml")),
            FileType::YAML => Some(OsStr::new("yml")),
            FileType::JSON => Some(OsStr::new("json")),
            FileType::JSONLD => Some(OsStr::new("jsonld")),
            FileType::N3 |
            FileType::NQuads |
            FileType::NTriples |
            FileType::RdfXml |
            FileType::TriG |
            FileType::Turtle => {
                self.oxi_graph_rdf_format()
                    .map(|f| OsStr::new(f.file_extension()))
            },
            FileType::JekyllConfig => Some(OsStr::new("yml")),
        }
    }

    /// Returns the file type associated with the given extension.
    pub fn from_extension(extension: &OsStr) -> Option<&'static FileType> {
        FILE_TYPE_MAP.get(extension).map(|f| &**f)
    }

    /// Returns the OxiGraph `RdfFormat` associated with the file
    /// type that's supported by the `BulkLoader`.
    /// At the moment, formats like `JSONLD` are not supported.
    pub fn oxi_graph_rdf_format(&self) -> Option<RdfFormat> {
        match self {
            FileType::N3 => Some(RdfFormat::N3),
            FileType::NQuads => Some(RdfFormat::NQuads),
            FileType::NTriples => Some(RdfFormat::NTriples),
            FileType::RdfXml => Some(RdfFormat::RdfXml),
            FileType::TriG => Some(RdfFormat::TriG),
            FileType::Turtle => Some(RdfFormat::Turtle),
            _ => None,
        }
    }

    /// Returns the MIME type associated with the file type.
    /// The format [IANA media type](https://tools.ietf.org/html/rfc2046).
    pub fn media_type(&self) -> Option<&'static str> {
        match self {
            FileType::Markdown => Some("text/markdown"),
            FileType::TOML => Some("application/toml"),
            FileType::YAML | FileType::JekyllConfig => {
                Some("application/x-yaml")
            },
            FileType::JSON => Some("application/json"),
            FileType::JSONLD => Some("application/ld+json"),
            FileType::N3 |
            FileType::NQuads |
            FileType::NTriples |
            FileType::RdfXml |
            FileType::TriG |
            FileType::Turtle => {
                self.oxi_graph_rdf_format().map(|f| f.media_type())
            },
        }
    }

    /// Returns true if the given file name matches the file type.
    pub fn is_of_type<P: AsRef<Path>>(&self, file_name: P) -> bool {
        let path = file_name.as_ref();
        if path.is_file() {
            if let Some(file_name) = self.file_name() {
                return file_name == path.file_name().unwrap();
            } else if let Some(extension) = self.extension() {
                if let Some(path_extension) = path.extension() {
                    return extension == path_extension;
                }
            }
        }
        false
    }

    /// Returns the specific file name if applicable.
    pub fn file_name(&self) -> Option<&'static OsStr> {
        match self {
            FileType::JekyllConfig => Some(OsStr::new(JEKYLL_CONFIG_FILE_NAME)),
            _ => None,
        }
    }

    /// Returns true if the file type has a specific file name.
    pub fn is_file_name(&self) -> bool { self.file_name().is_some() }

    /// Returns the specific file path if applicable.
    pub fn file_path(&self) -> Option<&Path> { self.file_name().map(Path::new) }

    /// Returns the file type associated with the given file name,
    /// if known.
    pub fn from_path<P: AsRef<Path>>(
        file_name: P,
    ) -> Option<&'static FileType> {
        // Convert the argument to a Path
        let path = file_name.as_ref();

        // First check if the file name is a specific file type like
        // `_config.yml` before checking the extension.
        if let Some(stem) = path.file_stem() {
            if stem.to_string_lossy().as_ref() == JEKYLL_CONFIG_FILE_NAME {
                return Some(&FileType::JekyllConfig);
            }
        }
        // Then check the extension.
        if let Some(extension) = path.extension() {
            if let Some(file_type) = FileType::from_extension(extension) {
                return Some(file_type);
            }
        }
        None
    }

    /// Helper function to check if a file matches the given file
    /// types.
    pub fn is_matching_file_type(
        path: &Path,
        types: FileTypeSlice<'_>,
    ) -> bool {
        if types.is_empty() {
            return false;
        }
        for file_type in types {
            if file_type.is_of_type(path) {
                return true;
            }
        }
        false
    }

    pub fn from_slice_to_cloned_vec<'a>(
        types: &'a [&'a FileType],
    ) -> Vec<FileType> {
        types.iter().map(|&t| *t).collect()
    }

    pub fn create_vec_of_references(types_vec: &[FileType]) -> Vec<&FileType> {
        types_vec.iter().collect()
    }

    pub fn ignore_crate_type_name(&self) -> String {
        self.name()
            .replace(" ", "_")
            .replace("/", "_")
            .replace("-", "_")
            .replace("_", "")
            .to_lowercase()
    }

    pub fn to_ignore_crate_type_globs(&self) -> Option<String> {
        if let Some(file_name) = self.file_name() {
            Some(format!(
                "{:}:{:}",
                self.ignore_crate_type_name(),
                file_name.to_string_lossy()
            ))
        } else {
            self.extension().map(|ext| {
                format!(
                    "{:}:*.{:}",
                    self.ignore_crate_type_name(),
                    ext.to_string_lossy()
                )
            })
        }
    }

    /// Adds the file type to the ignore crate types builder.
    pub fn to_ignore_crate_type(
        &self,
        ignore_crate_types_builder: &mut ignore::types::TypesBuilder,
    ) -> anyhow::Result<()> {
        if let Some(globs) = self.to_ignore_crate_type_globs() {
            ignore_crate_types_builder.add_def(&globs)?;
            let name = self.ignore_crate_type_name();
            ignore_crate_types_builder.select(name.as_str());
        }
        Ok(())
    }
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:}", self.name())?;
        if self.is_file_name() {
            write!(f, " ({:?})", self.file_name().unwrap())?;
        } else {
            write!(
                f,
                " (*.{:})",
                self.extension().unwrap_or_default().to_string_lossy()
            )?;
        };
        let globs = self.to_ignore_crate_type_globs();
        if let Some(globs) = globs {
            write!(f, " {:?}", globs)?;
        }
        Ok(())
    }
}

impl AsRef<str> for FileType {
    fn as_ref(&self) -> &str { self.name() }
}

impl From<&Path> for &'static FileType {
    fn from(path: &Path) -> Self { FileType::from_path(path).unwrap() }
}

static FILE_TYPE_MAP: LazyLock<HashMap<&'static OsStr, &'static FileType>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert(OsStr::new("md"), &FileType::Markdown);
        map.insert(OsStr::new("toml"), &FileType::TOML);
        map.insert(OsStr::new("yml"), &FileType::YAML);
        map.insert(OsStr::new("json"), &FileType::JSON);
        map.insert(OsStr::new("jsonld"), &FileType::JSONLD);
        map.insert(OsStr::new("n3"), &FileType::N3);
        map.insert(OsStr::new("nquads"), &FileType::NQuads);
        map.insert(OsStr::new("ntriples"), &FileType::NTriples);
        map.insert(OsStr::new("rdfxml"), &FileType::RdfXml);
        map.insert(OsStr::new("trig"), &FileType::TriG);
        map.insert(OsStr::new("turtle"), &FileType::Turtle);
        map.insert(OsStr::new("yml"), &FileType::JekyllConfig);
        map
    });
