use {
    crate::{model::Book, util::generate_uuid_named_node},
    oxigraph::store::Store,
    oxrdf::{GraphName, Literal, NamedNode, Quad, QuadRef},
    std::sync::Arc,
    tracing::trace,
};

/// The documentation model holds the data that needs to be
/// documented. It is a wrapper around an OxiGraph store.
/// The store does not necessarily hold all data from the source, but
/// only the data that's needed by the output generators to generate
/// the documentation. This documentation data's structure is
/// primarily defined by the grapharch ontology.
#[derive(Clone)]
pub struct DocumentationModel {
    pub store: Arc<Store>,
}

impl DocumentationModel {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self { store: Arc::new(Store::new()?) })
    }

    /// Add an interesting quad to the doc model that needs to be
    /// documented. Usually this is about quads whose predicate is
    /// rdf:type so it's primarily the subject that's interesting.
    pub async fn add_quad(
        &mut self,
        quad: QuadRef<'_>,
    ) -> anyhow::Result<()> {
        self.store.insert(quad)?;
        trace!("Added quad to docmodel: {:?}", quad);
        Ok(())
    }

    pub fn get_store(&self) -> &Store { &self.store }

    pub fn add_book(&self, book: Book) -> anyhow::Result<()> {
        let book_node = generate_uuid_named_node()?;
        let graph_name = GraphName::DefaultGraph;

        let title_quad = Quad::new(
            book_node.clone(),
            NamedNode::new("http://purl.org/dc/terms/title")?,
            Literal::new_simple_literal(&book.title),
            graph_name.clone(),
        );

        let author_quads = book.authors.iter().map(|author| {
            Quad::new(
                book_node.clone(),
                NamedNode::new("http://purl.org/dc/terms/creator")
                    .unwrap(),
                Literal::new_simple_literal(author),
                graph_name.clone(),
            )
        });

        self.store.insert(&title_quad)?;
        for author_quad in author_quads {
            self.store.insert(&author_quad)?;
        }

        trace!("Added book to docmodel: {:?}", book);
        Ok(())
    }
}

impl std::fmt::Debug for DocumentationModel {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "DocumentationModel")
    }
}
