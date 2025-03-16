use {
    crate::{
        generator::Generator,
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
    output_dir: PathBuf,
    pdf_output: PathBuf,
    /// Metadata about discovered fonts.
    font_book:  LazyHash<FontBook>,
    /// Locations of and storage for lazily loaded fonts.
    fonts:      Vec<FontSlot>,
    library:    LazyHash<Library>,
    /// The temp directory for storing the Typst source files
    #[allow(dead_code)]
    temp_dir:   Option<tempfile::TempDir>,
    /// The current date and time
    #[allow(dead_code)]
    now:        Now,
}

impl TypstGenerator {
    /// Creates a new Typst generator that will generate Typst source files
    /// in the given directory (or a temporary directory if none is specified)
    /// and compile them to PDF at the specified location (or ./output.pdf if
    /// none specified).
    pub fn new<P1: AsRef<Path>, P2: AsRef<Path>>(
        output_dir: Option<P1>,
        pdf_output: Option<P2>,
    ) -> anyhow::Result<Self> {
        let (output_dir, temp_dir) = match output_dir {
            Some(p) => (p.as_ref().to_path_buf(), None),
            None => {
                let temp_dir = tempfile::tempdir()?;
                (temp_dir.path().to_path_buf(), Some(temp_dir))
            },
        };

        let pdf_output = pdf_output
            .map(|p| p.as_ref().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("output.pdf"));

        let fonts = Fonts::searcher().include_system_fonts(true).search();

        Ok(Self {
            output_dir,
            pdf_output,
            font_book: LazyHash::new(fonts.book),
            fonts: fonts.fonts,
            library: LazyHash::new(Library::default()),
            temp_dir,
            now: Now::System(OnceLock::new()),
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

        // Create a template file
        let template_content = r#"#let project(
          title: "",
          subtitle: "",
          authors: "",
          body,
        ) = {
          set document(title: title, author: authors)
          set page(numbering: "1", number-align: center)
          set text(font: "New Computer Modern")
          set heading(numbering: "1.")

          align(center)[
            #block(text(weight: "bold", size: 24pt)[#title])
            #if subtitle != "" [
              #block(text(style: "italic", size: 14pt)[#subtitle])
            ]
            #block(text(size: 12pt)[#authors])
          ]

          body
        }"#;
        fs::write(
            self.output_dir.join("template.typ"),
            template_content,
        )?;

        // Process each book and create book files
        for book in &books {
            // Create a file for this book
            let book_file = self.output_dir.join(format!(
                "{}.typ",
                book.title.as_deref().unwrap_or("Untitled")
            ));
            let mut book_content = String::new();
            book_content.push_str("#import \"template.typ\": project\n\n");

            // Start the project function
            book_content.push_str(&format!(
                "#project(\n  title: \"{}\",\n",
                book.title.as_deref().unwrap_or("Untitled")
            ));

            if let Some(ref subtitle) = book.subtitle {
                book_content
                    .push_str(&format!("  subtitle: \"{}\",\n", subtitle));
            } else {
                book_content.push_str("  subtitle: \"\",\n");
            }

            book_content.push_str(&format!(
                "  authors: \"{}\",\n)[",
                book.authors.join(", ")
            ));

            // Get sections for this book
            let sections = book.get_sections().unwrap_or_default();

            if sections.is_empty() {
                // Add a default content if no sections are found
                book_content.push_str("\n  = Default Content\n\n");
                book_content.push_str(
                    "  This is a default document generated by GraphArch.\n\n",
                );
                book_content
                    .push_str("  No content was found in the input file.\n\n");
            } else {
                // Process sections normally
                for section in sections {
                    book_content.push_str(&format!(
                        "\n  = {}\n\n",
                        section.title.as_deref().unwrap_or("Untitled")
                    ));

                    if let Some(ref description) = section.description {
                        book_content
                            .push_str(&format!("  {}\n\n", description));
                    }

                    // Get chapters for this section
                    for chapter in section.get_chapters().unwrap_or_default() {
                        book_content.push_str(&format!(
                            "  == {}\n\n",
                            chapter.title.as_deref().unwrap_or("Untitled")
                        ));
                        book_content.push_str(&format!(
                            "  {}\n\n",
                            chapter.content.as_deref().unwrap_or("")
                        ));
                    }
                }
            }

            // Close the project function
            book_content.push_str("]\n");

            // Write the book file
            fs::write(&book_file, &book_content)?;
        }

        // Write the main file
        let mut main_content = String::new();

        // Include each book file
        for book in &books {
            let book_filename = format!(
                "{}.typ",
                book.title.as_deref().unwrap_or("Untitled")
            );
            main_content.push_str(&format!("#include \"{}\"\n", book_filename));
        }

        fs::write(self.output_dir.join("main.typ"), &main_content)?;

        Ok(())
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
