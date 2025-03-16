use {
    super::{
        Buildable,
        Model,
        element::{Element, ElementRef},
        section::Section,
    },
    oxrdf::{Literal, NamedNode},
    std::sync::Arc,
    tracing::{info, trace},
};

#[derive(Debug, Clone)]
pub struct Book {
    element_ref:    ElementRef,
    pub title:      Option<String>,
    pub subtitle:   Option<String>,
    pub authors:    Vec<String>,
    pub repository: Option<String>,
    pub url:        Option<String>,
    pub sections:   Vec<Section>,
}

impl Element for Book {
    type Builder = BookBuilder;

    fn get_element_ref(&self) -> &ElementRef { &self.element_ref }

    fn builder(element_ref: ElementRef) -> anyhow::Result<Self::Builder> {
        Ok(BookBuilder::new(element_ref))
    }
}

#[derive(Clone)]
pub struct BookBuilder {
    element_ref: ElementRef,
    title:       Option<String>,
    subtitle:    Option<String>,
    authors:     Vec<String>,
    repository:  Option<String>,
    url:         Option<String>,
}

impl Buildable<Book> for BookBuilder {
    fn new(element_ref: ElementRef) -> Self {
        Self {
            element_ref,
            title: None,
            subtitle: None,
            authors: Vec::new(),
            repository: None,
            url: None,
        }
    }

    fn get_element_ref(&self) -> &ElementRef { &self.element_ref }

    fn build(&mut self) -> anyhow::Result<Book> {
        // Add type quad
        self.insert_type("http://example.org/Book")?;

        // Add title quad if present
        if let Some(ref title) = self.title {
            self.insert_object_literal(
                NamedNode::new("http://purl.org/dc/terms/title")?,
                Literal::new_simple_literal(title),
            )?;
        }

        // Add author quads
        for author in &self.authors {
            self.insert_object_literal(
                NamedNode::new("http://purl.org/dc/terms/creator")?,
                Literal::new_simple_literal(author),
            )?;
        }

        // Add optional fields
        if let Some(ref repository) = self.repository {
            self.insert_object_literal(
                NamedNode::new("http://example.com/schema/repository")?,
                Literal::new_simple_literal(repository),
            )?;
        }

        if let Some(ref url) = self.url {
            self.insert_object_literal(
                NamedNode::new("http://example.com/schema/url")?,
                Literal::new_simple_literal(url),
            )?;
        }

        Ok(Book {
            element_ref: self.element_ref.clone(),
            title:       self.title.clone(),
            subtitle:    self.subtitle.clone(),
            authors:     self.authors.clone(),
            repository:  self.repository.clone(),
            url:         self.url.clone(),
            sections:    Vec::new(),
        })
    }
}

impl BookBuilder {
    /// Sets the title for the book.
    pub fn title(mut self, title: Option<String>) -> Self {
        self.title = title;
        self
    }

    /// Sets the subtitle for the book.
    pub fn subtitle(mut self, subtitle: Option<String>) -> Self {
        self.subtitle = subtitle;
        self
    }

    /// Adds an author to the book.
    pub fn author(mut self, author: Option<String>) -> Self {
        if let Some(author) = author {
            self.authors.push(author);
        }
        self
    }

    /// Sets the repository URL for the book.
    pub fn repository(mut self, repository: Option<String>) -> Self {
        self.repository = repository;
        self
    }

    /// Sets the URL for the book.
    pub fn url(mut self, url: Option<String>) -> Self {
        self.url = url;
        self
    }
}

impl Book {
    /// Get all sections for a given book
    /// TODO: Move this to the Book class, it should just be called
    /// "Book::get_sections" returning a vector of Sections
    pub fn get_sections(&self) -> anyhow::Result<Vec<Section>> {
        let query = format!(
            r#"
            PREFIX dc: <http://purl.org/dc/terms/>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            SELECT DISTINCT ?graph ?iri ?title ?description WHERE {{
                GRAPH ?graph {{
                    ?iri rdf:type <http://example.org/Section> ;
                            dc:title ?title ;
                            <http://example.org/schema/belongsTo> {} .
                    OPTIONAL {{ ?iri dc:description ?description }}
                }}
            }}
            "#,
            self.get_named_node(),
        );

        let results = self.get_model().get_store().query(&query)?;
        let mut sections = Vec::new();

        if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                let solution = solution?;
                let section = Section {
                    element_ref: ElementRef::from_solution(
                        self.get_model(),
                        &solution,
                    )?,
                    title:       solution.get("title").map(|s| s.to_string()),
                    description: solution
                        .get("description")
                        .map(|s| s.to_string()),
                };
                sections.push(section);
            }
        }

        // Return the sections even if empty
        Ok(sections)
    }

    pub fn get_book(model: Arc<Model>, title: &str) -> anyhow::Result<Book> {
        Self::get_books(model)?
            .into_iter() // Change from iter() to into_iter() to consume the vector
            .find(|book| book.title == Some(title.to_string()))
            .ok_or_else(|| {
                anyhow::anyhow!("Book not found for title: {}", title)
            })
    }

    /// Get all books in the given documentation model
    pub fn get_books(model: Arc<Model>) -> anyhow::Result<Vec<Book>> {
        info!("Retrieving all books from documentation model");
        let query = r#"
            PREFIX dc: <http://purl.org/dc/terms/>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            SELECT DISTINCT ?graph ?iri ?title ?subtitle ?author WHERE {
                GRAPH ?graph {
                    ?book rdf:type <http://example.org/Book> ;
                          dc:title ?title ;
                          dc:creator ?author .
                    OPTIONAL { ?book dc:subtitle ?subtitle }
                }
            }
        "#;

        let results = model.get_store().query(query)?;
        let mut books = Vec::new();

        if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                let solution = solution?;
                let book = Book {
                    element_ref: ElementRef::from_solution(&model, &solution)?,
                    title:       solution.get("title").map(|s| s.to_string()),
                    subtitle:    solution
                        .get("subtitle")
                        .map(|s| s.to_string()),
                    authors:     vec![
                        solution.get("author").unwrap().to_string(),
                    ],
                    repository:  None,
                    url:         None,
                    sections:    Vec::new(),
                };
                books.push(book);
            }
        }

        trace!("Found {} books", books.len());
        Ok(books)
    }
}
