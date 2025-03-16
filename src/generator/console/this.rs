use {
    crate::{
        generator::Generator,
        model::{Book, Chapter, Model, Section},
    },
    console::Style,
    std::{
        io::{self, Write},
        sync::Arc,
    },
    tracing::info,
};

/// A generator that outputs documentation to the console using ANSI colors
pub struct ConsoleGenerator {
    writer: Box<dyn Write>,
}

impl std::fmt::Debug for ConsoleGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsoleGenerator").finish_non_exhaustive()
    }
}

impl Default for ConsoleGenerator {
    fn default() -> Self { Self::new() }
}

impl ConsoleGenerator {
    pub fn new() -> Self { Self { writer: Box::new(io::stdout()) } }

    fn write_book(&mut self, book: &Book) -> io::Result<()> {
        let title_style = Style::new().bold().green();
        let subtitle_style = Style::new().italic().blue();
        let author_style = Style::new().yellow();

        writeln!(
            self.writer,
            "\n{}",
            title_style.apply_to(book.title.as_deref().unwrap_or("Untitled"))
        )?;
        if let Some(subtitle) = &book.subtitle {
            writeln!(
                self.writer,
                "{}",
                subtitle_style.apply_to(subtitle)
            )?;
        }
        writeln!(
            self.writer,
            "{}",
            author_style.apply_to(format!("By {}", book.authors.join(", ")))
        )?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn write_section(&mut self, section: &Section) -> io::Result<()> {
        let title_style = Style::new().bold().cyan();
        let desc_style = Style::new().italic().white();

        writeln!(
            self.writer,
            "{}",
            title_style
                .apply_to(section.title.as_deref().unwrap_or("Untitled"))
        )?;
        if let Some(desc) = &section.description {
            writeln!(self.writer, "{}", desc_style.apply_to(desc))?;
        }
        writeln!(self.writer)?;
        Ok(())
    }

    fn write_chapter(&mut self, chapter: &Chapter) -> io::Result<()> {
        let title_style = Style::new().bold().magenta();
        let content_style = Style::new().white();

        writeln!(
            self.writer,
            "{}",
            title_style
                .apply_to(chapter.title.as_deref().unwrap_or("Untitled"))
        )?;
        writeln!(
            self.writer,
            "{}",
            content_style
                .apply_to(chapter.content.as_deref().unwrap_or("Untitled"))
        )?;
        writeln!(self.writer)?;
        Ok(())
    }
}

impl Generator for ConsoleGenerator {
    fn generate(&mut self, doc_model: Arc<Model>) -> anyhow::Result<()> {
        info!("Generating console output");

        // Get all books using the type-safe API
        let books = Book::get_books(doc_model)?;
        for book in books {
            self.write_book(&book)?;

            // Get sections for this book
            let sections = book.get_sections()?;
            for section in sections {
                self.write_section(&section)?;

                // Get chapters for this section
                let chapters = section.get_chapters()?;
                for chapter in chapters {
                    self.write_chapter(&chapter)?;
                }
            }
        }

        info!("Console output generation completed");
        Ok(())
    }
}
