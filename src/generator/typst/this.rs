use {
    crate::{
        generator::{Generator, typst::markdown::MarkdownToTypstConverter},
        model::{Book, Buildable, Element, Model},
    },
    chrono::{DateTime, Datelike, FixedOffset, Local, Utc},
    std::{
        fs::{self},
        path::{Path, PathBuf},
        sync::{Arc, OnceLock},
    },
    tracing::info,
    typst::{
        Library,
        World,
        diag::FileError,
        foundations::{Bytes, Datetime},
        layout::PagedDocument,
        syntax::{FileId, Source, VirtualPath},
        text::{Font, FontBook},
        utils::LazyHash,
    },
    typst_kit::fonts::{FontSlot, Fonts},
    typst_pdf::PdfOptions,
};

/// The current date and time.
enum Now {
    /// The date and time if the environment `SOURCE_DATE_EPOCH` is set.
    /// Used for reproducible builds.
    #[allow(dead_code)]
    Fixed(DateTime<Utc>),
    /// The current date and time if the time is not externally fixed.
    System(OnceLock<DateTime<Utc>>),
}

pub struct TypstGenerator {
    output_dir:         PathBuf,
    pdf_output:         PathBuf,
    /// Metadata about discovered fonts.
    font_book:          LazyHash<FontBook>,
    /// Locations of and storage for lazily loaded fonts.
    fonts:              Vec<FontSlot>,
    library:            LazyHash<Library>,
    /// The temp directory for storing the Typst source files
    #[allow(dead_code)]
    temp_dir:           Option<tempfile::TempDir>,
    /// The current date and time
    #[allow(dead_code)]
    now:                Now,
    markdown_converter: MarkdownToTypstConverter,
}

impl TypstGenerator {
    /// Create a new generator with the given output directory
    pub fn new(output_dir: PathBuf, pdf_output: PathBuf) -> Self {
        let fonts = Fonts::searcher().include_system_fonts(true).search();

        Self {
            output_dir,
            pdf_output,
            font_book: LazyHash::new(fonts.book),
            fonts: fonts.fonts,
            library: LazyHash::new(Library::default()),
            temp_dir: None,
            now: Now::System(OnceLock::new()),
            markdown_converter: MarkdownToTypstConverter::new(),
        }
    }

    /// Create a new generator with a temporary output directory
    pub fn with_temp_dir(pdf_output: PathBuf) -> anyhow::Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let output_dir = temp_dir.path().to_path_buf();
        let fonts = Fonts::searcher().include_system_fonts(true).search();

        Ok(Self {
            output_dir,
            pdf_output,
            font_book: LazyHash::new(fonts.book),
            fonts: fonts.fonts,
            library: LazyHash::new(Library::default()),
            temp_dir: Some(temp_dir),
            now: Now::System(OnceLock::new()),
            markdown_converter: MarkdownToTypstConverter::new(),
        })
    }

    fn compile_to_pdf(&self) -> anyhow::Result<()> {
        use super::error::eco_vec_into_anyhow;

        info!(
            "Starting Typst compilation from directory: {}",
            self.output_dir.display()
        );

        // List files in the output directory to verify they exist
        if let Ok(entries) = fs::read_dir(&self.output_dir) {
            info!("Files in output directory:");
            for entry in entries.flatten() {
                info!("  {}", entry.path().display());
            }
        }

        let result = typst::compile::<PagedDocument>(self);
        let document = match result.output {
            Ok(doc) => doc,
            Err(err) => {
                info!("Compilation failed with errors: {:?}", err);
                return Err(eco_vec_into_anyhow(err));
            },
        };

        // Handle warnings
        for warning in result.warnings {
            info!("Typst warning: {:?}", warning);
        }

        info!("Compilation successful, generating PDF");
        let pdf_data = typst_pdf::pdf(&document, &PdfOptions::default())
            .map_err(|e| anyhow::anyhow!("Failed to generate PDF: {:?}", e))?;

        info!("Writing PDF to {}", self.pdf_output.display());
        fs::write(&self.pdf_output, pdf_data)?;
        info!(
            "Successfully compiled Typst files to PDF at {}",
            self.pdf_output.display()
        );
        Ok(())
    }
}

impl World for TypstGenerator {
    fn library(&self) -> &LazyHash<Library> { &self.library }

    fn book(&self) -> &LazyHash<FontBook> { &self.font_book }

    fn main(&self) -> FileId { FileId::new(None, VirtualPath::new("main.typ")) }

    fn source(&self, id: FileId) -> Result<Source, FileError> {
        let vpath = id.vpath();
        let path_str = vpath.as_rooted_path().to_string_lossy();
        let rel_path = path_str.trim_start_matches('/');
        let path = self.output_dir.join(rel_path);

        info!(
            "Typst requesting source file: {:?} (resolved to {})",
            vpath,
            path.display()
        );

        fs::read_to_string(&path)
            .map(|content| {
                info!(
                    "Successfully read source file: {} ({} bytes)",
                    path.display(),
                    content.len()
                );
                Source::new(id, content)
            })
            .map_err(|err| {
                info!(
                    "Failed to read source file: {} ({})",
                    path.display(),
                    err
                );
                FileError::from_io(err, &path)
            })
    }

    fn file(&self, id: FileId) -> Result<Bytes, FileError> {
        let vpath = id.vpath();
        let path_str = vpath.as_rooted_path().to_string_lossy();
        let rel_path = path_str.trim_start_matches('/');
        let path = self.output_dir.join(rel_path);

        info!(
            "Typst requesting file: {:?} (resolved to {})",
            vpath,
            path.display()
        );

        fs::read(&path)
            .map(|bytes| {
                info!(
                    "Successfully read file: {} ({} bytes)",
                    path.display(),
                    bytes.len()
                );
                Bytes::new(bytes.into_boxed_slice())
            })
            .map_err(|err| {
                info!(
                    "Failed to read file: {} ({})",
                    path.display(),
                    err
                );
                FileError::from_io(err, &path)
            })
    }

    /// Try to access the font with the given index in the font book.
    fn font(&self, index: usize) -> Option<Font> { self.fonts[index].get() }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let now = match &self.now {
            Now::Fixed(time) => time,
            Now::System(time) => time.get_or_init(Utc::now),
        };

        // The time with the specified UTC offset, or within the local time
        // zone.
        let with_offset = match offset {
            None => now.with_timezone(&Local).fixed_offset(),
            Some(hours) => {
                let seconds = i32::try_from(hours).ok()?.checked_mul(3600)?;
                now.with_timezone(&FixedOffset::east_opt(seconds)?)
            },
        };

        Datetime::from_ymd(
            with_offset.year(),
            with_offset.month().try_into().ok()?,
            with_offset.day().try_into().ok()?,
        )
    }
}

impl std::fmt::Debug for TypstGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypstGenerator")
            .field("output_dir", &self.output_dir)
            .field("pdf_output", &self.pdf_output)
            .finish()
    }
}

impl TypstGenerator {
    fn generate_typst_files(
        &self,
        doc_model: Arc<Model>,
    ) -> anyhow::Result<()> {
        // Create output directory if it doesn't exist
        fs::create_dir_all(&self.output_dir)?;

        // Also create a debug directory in the current working directory
        let debug_dir = std::path::Path::new("typst_debug");
        fs::create_dir_all(debug_dir)?;

        // Get all books from the documentation model
        let mut books = Book::get_books(doc_model.clone())?;

        // If no books were found, create a default one
        if books.is_empty() {
            info!(
                "No books found in the documentation model, creating a \
                 default book"
            );

            // Create a book builder directly using builder_in_model and chain
            // the method calls
            let mut book_builder = Book::builder_in_model::<Book>(&doc_model)?
                .title(Some("Documentation".to_string()))
                .author(Some("GraphArch".to_string()));

            // Build the book
            let default_book = book_builder.build()?;

            books.push(default_book);
        }

        // Create a template file - simplified for debugging
        let template_content = self.generate_template_content();
        fs::write(
            self.output_dir.join("template.typ"),
            &template_content,
        )?;
        // Also write to debug directory
        fs::write(debug_dir.join("template.typ"), template_content)?;

        // Process each book and create book files
        for book in &books {
            // Create a file for this book
            let book_title = book.title.as_deref().unwrap_or("Untitled");
            let sanitized_filename = self.sanitize_filename(book_title);

            // Generate files for each component of the book
            self.generate_book_files(book, &sanitized_filename, debug_dir)?;
        }

        // Write the main file - simplified for debugging
        let book_title = books[0].title.as_deref().unwrap_or("Untitled");
        let sanitized_filename = self.sanitize_filename(book_title);

        let main_content = self.generate_main_content(&sanitized_filename);

        fs::write(self.output_dir.join("main.typ"), &main_content)?;
        // Also write to debug directory
        fs::write(debug_dir.join("main.typ"), &main_content)?;

        Ok(())
    }

    /// Sanitize a string to be used as a filename
    fn sanitize_filename(&self, name: &str) -> String {
        name.to_lowercase()
            .replace(' ', "-")
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "")
    }

    /// Generate files for each component of a book
    fn generate_book_files(
        &self,
        book: &Book,
        book_filename: &str,
        debug_dir: &Path,
    ) -> anyhow::Result<()> {
        // Create book directory
        let book_dir = self.output_dir.join(book_filename);
        let debug_book_dir = debug_dir.join(book_filename);
        fs::create_dir_all(&book_dir)?;
        fs::create_dir_all(&debug_book_dir)?;

        // Create book index file
        // Sanitize title and authors
        let sanitized_title = book
            .title
            .as_deref()
            .unwrap_or("Untitled")
            .replace("\"", "");

        let sanitized_subtitle =
            book.subtitle.as_deref().unwrap_or("").replace("\"", "");

        let sanitized_authors = if book.authors.is_empty() {
            String::new()
        } else {
            book.authors.join(", ")
        }
        .replace("\"", "");

        let book_index_content = format!(
            "#import \"../template.typ\": *\n\n#show: project(\"{}\", \"{}\", \
             \"{}\")\n\n",
            sanitized_title, sanitized_subtitle, sanitized_authors
        );

        // Get sections for this book
        let sections = book.get_sections().unwrap_or_default();

        if sections.is_empty() {
            // Add a default content file if no sections are found
            let default_content = "= Default Content\n\nThis is a default \
                                   document generated by GraphArch.\n\nNo \
                                   content was found in the input file.\n\n";
            let default_file = "default.typ";

            fs::write(book_dir.join(default_file), default_content)?;
            fs::write(debug_book_dir.join(default_file), default_content)?;

            // Include default content in book index
            let mut updated_index = book_index_content.clone();
            updated_index.push_str(&format!("#include \"{}\"", default_file));

            fs::write(book_dir.join("index.typ"), &updated_index)?;
            fs::write(debug_book_dir.join("index.typ"), &updated_index)?;
        } else {
            // Process each section
            let mut section_includes = String::new();

            for (section_idx, section) in sections.iter().enumerate() {
                let section_title = section
                    .title
                    .as_deref()
                    .unwrap_or("Untitled")
                    .trim_matches('"');
                let section_dirname = format!("section-{}", section_idx + 1);

                // Create section directory
                let section_dir = book_dir.join(&section_dirname);
                let debug_section_dir = debug_book_dir.join(&section_dirname);
                fs::create_dir_all(&section_dir)?;
                fs::create_dir_all(&debug_section_dir)?;

                // Create section index file with its heading and description
                let mut section_content = format!("= {}\n\n", section_title);

                if let Some(ref description) = section.description {
                    section_content.push_str(&format!(
                        "{}\n\n",
                        description.trim_matches('"')
                    ));
                }

                // Get chapters for this section
                let chapters = section.get_chapters().unwrap_or_default();

                // Create chapter includes for section index
                let mut chapter_includes = String::new();

                // Process each chapter
                for (chapter_idx, chapter) in chapters.iter().enumerate() {
                    let chapter_title = chapter
                        .title
                        .as_deref()
                        .unwrap_or("Untitled")
                        .trim_matches('"');
                    let chapter_dirname =
                        format!("chapter-{}", chapter_idx + 1);

                    // Create chapter directory
                    let chapter_dir = section_dir.join(&chapter_dirname);
                    let debug_chapter_dir =
                        debug_section_dir.join(&chapter_dirname);
                    fs::create_dir_all(&chapter_dir)?;
                    fs::create_dir_all(&debug_chapter_dir)?;

                    if let Some(ref chapter_content) = chapter.content {
                        // Process the content using the
                        // MarkdownToTypstConverter
                        let raw_content = chapter_content
                            .trim_matches('"')
                            .replace("\\n", "\n");

                        // Use the markdown converter to process the content
                        let processed_content =
                            self.markdown_converter.convert(&raw_content);

                        // Create chapter index file
                        let chapter_index_content = format!(
                            "== {}\n\n{}",
                            chapter_title, processed_content
                        );

                        fs::write(
                            chapter_dir.join("index.typ"),
                            &chapter_index_content,
                        )?;
                        fs::write(
                            debug_chapter_dir.join("index.typ"),
                            &chapter_index_content,
                        )?;
                    } else {
                        // Empty chapter
                        let chapter_index_content = format!(
                            "== {}\n\nNo content available.\n\n",
                            chapter_title
                        );

                        fs::write(
                            chapter_dir.join("index.typ"),
                            &chapter_index_content,
                        )?;
                        fs::write(
                            debug_chapter_dir.join("index.typ"),
                            &chapter_index_content,
                        )?;
                    }

                    // Add chapter include to section index
                    chapter_includes.push_str(&format!(
                        "#include \"{}/index.typ\"\n\n",
                        chapter_dirname
                    ));
                }

                // Write section index with chapter includes
                let full_section_content =
                    format!("{}\n{}", section_content, chapter_includes);
                fs::write(
                    section_dir.join("index.typ"),
                    &full_section_content,
                )?;
                fs::write(
                    debug_section_dir.join("index.typ"),
                    &full_section_content,
                )?;

                // Add section include to book index
                section_includes.push_str(&format!(
                    "#include \"{}/index.typ\"\n\n",
                    section_dirname
                ));
            }

            // Write book index with section includes
            let full_book_content =
                format!("{}\n{}", book_index_content, section_includes);
            fs::write(book_dir.join("index.typ"), &full_book_content)?;
            fs::write(
                debug_book_dir.join("index.typ"),
                &full_book_content,
            )?;
        }

        Ok(())
    }

    /// Generate the main content file
    fn generate_main_content(&self, book_name: &str) -> String {
        format!(
            "#import \"template.typ\": *\n\n#show: project(\"{}\", \"\", \
             \"GraphArch\")\n\n#include \"{}/index.typ\"",
            book_name, book_name
        )
    }

    fn generate_template_content(&self) -> String {
        r#"#let project(title, subtitle, authors) = doc => {
  set document(title: title, author: authors)
  set page(numbering: "1", number-align: center)
  set text(font: "New Computer Modern")
  set heading(numbering: "1.")

  align(center)[
    #text(weight: "bold", size: 24pt)[#title]
    #if subtitle != "" [
      #text(style: "italic", size: 14pt)[#subtitle]
    ]
    #text(size: 12pt)[#authors]
  ]

  doc
}"#
        .to_string()
    }
}

impl Generator for TypstGenerator {
    fn generate(&mut self, doc_model: Arc<Model>) -> anyhow::Result<()> {
        // First generate the Typst source files
        self.generate_typst_files(doc_model)?;

        // Always compile to PDF since we now always have a valid path
        self.compile_to_pdf()?;

        Ok(())
    }
}
