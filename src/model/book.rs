use {
    anyhow::Result,
    oxigraph::{
        model::{GraphName, Literal, NamedNode, Quad},
        store::Store,
    },
    uuid::Uuid,
};

#[derive(Debug)]
#[allow(unused)]
pub struct Book {
    pub title:           String,
    pub subtitle:        Option<String>,
    pub authors:         Vec<String>,
    pub title_separator: Option<String>,
    pub repository:      Option<String>,
    pub url:             Option<String>,
    // Add other fields as necessary
}

pub struct BookBuilder {
    title:           String,
    subtitle:        Option<String>,
    authors:         Vec<String>,
    title_separator: Option<String>,
    repository:      Option<String>,
    url:             Option<String>,
    // Add other fields as necessary
}

impl BookBuilder {
    /// Creates a new BookBuilder with the specified title.
    pub fn new(title: String) -> Self {
        Self {
            title,
            subtitle: None,
            authors: Vec::new(),
            title_separator: None,
            repository: None,
            url: None,
            // Initialize other fields as necessary
        }
    }

    /// Sets the subtitle for the book.
    pub fn subtitle(mut self, subtitle: String) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    /// Adds an author to the book.
    pub fn author(mut self, author: String) -> Self {
        self.authors.push(author);
        self
    }

    /// Sets the title separator for the book.
    pub fn title_separator(mut self, separator: String) -> Self {
        self.title_separator = Some(separator);
        self
    }

    /// Sets the repository URL for the book.
    pub fn repository(mut self, repository: String) -> Self {
        self.repository = Some(repository);
        self
    }

    /// Sets the URL for the book.
    pub fn url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    // Add other builder methods as necessary

    /// Builds the Book and inserts its data as RDF quads into the
    /// provided Store.
    pub fn build(self, store: &Store) -> Result<Book> {
        let book_uuid = Uuid::new_v4();
        let book_node = NamedNode::new(format!("urn:uuid:{}", book_uuid))?;
        let graph_name = GraphName::DefaultGraph;

        let title_quad = Quad::new(
            book_node.clone(),
            NamedNode::new("http://purl.org/dc/terms/title")?,
            Literal::new_simple_literal(&self.title),
            graph_name.clone(),
        );

        store.insert(&title_quad)?;

        for author in &self.authors {
            let author_quad = Quad::new(
                book_node.clone(),
                NamedNode::new("http://purl.org/dc/terms/creator")?,
                Literal::new_simple_literal(author),
                graph_name.clone(),
            );
            store.insert(&author_quad)?;
        }

        if let Some(separator) = &self.title_separator {
            let separator_quad = Quad::new(
                book_node.clone(),
                NamedNode::new("http://example.com/schema/titleSeparator")?,
                Literal::new_simple_literal(separator),
                graph_name.clone(),
            );
            store.insert(&separator_quad)?;
        }

        if let Some(repository) = &self.repository {
            let repository_quad = Quad::new(
                book_node.clone(),
                NamedNode::new("http://example.com/schema/repository")?,
                Literal::new_simple_literal(repository),
                graph_name.clone(),
            );
            store.insert(&repository_quad)?;
        }

        if let Some(url) = &self.url {
            let url_quad = Quad::new(
                book_node.clone(),
                NamedNode::new("http://example.com/schema/url")?,
                Literal::new_simple_literal(url),
                graph_name.clone(),
            );
            store.insert(&url_quad)?;
        }

        Ok(Book {
            title:           self.title,
            subtitle:        self.subtitle,
            authors:         self.authors,
            title_separator: self.title_separator,
            repository:      self.repository,
            url:             self.url,
        })
    }
}
