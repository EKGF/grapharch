use {
    super::{Buildable, Element, chapter::Chapter, element::ElementRef},
    anyhow::Result,
    oxrdf::Literal,
};

#[derive(Debug, Clone)]
pub struct Section {
    pub(super) element_ref: ElementRef,
    pub title:              Option<String>,
    pub description:        Option<String>,
}

impl Element for Section {
    type Builder = SectionBuilder;

    fn builder(element_ref: ElementRef) -> anyhow::Result<Self::Builder> {
        Ok(SectionBuilder::new(element_ref))
    }

    fn get_element_ref(&self) -> &ElementRef { &self.element_ref }
}

pub struct SectionBuilder {
    element_ref: ElementRef,
    title:       Option<String>,
    description: Option<String>,
}

impl Buildable<Section> for SectionBuilder {
    fn get_element_ref(&self) -> &ElementRef { &self.element_ref }

    fn new(element_ref: ElementRef) -> Self {
        Self { element_ref, title: None, description: None }
    }

    fn build(&mut self) -> Result<Section> {
        let section = Section {
            element_ref: self.element_ref.clone(),
            title:       self.title.clone(),
            description: self.description.clone(),
        };

        self.insert_type("http://example.org/Section")?;

        if let Some(title) = &self.title {
            self.insert_object_literal(
                oxrdf::NamedNode::new("http://purl.org/dc/terms/title")?,
                Literal::new_simple_literal(title),
            )?;
        }

        if let Some(description) = &self.description {
            self.insert_object_literal(
                oxrdf::NamedNode::new("http://purl.org/dc/terms/description")?,
                Literal::new_simple_literal(description),
            )?;
        }

        Ok(section)
    }
}

impl SectionBuilder {
    pub fn title(mut self, title: Option<String>) -> Self {
        self.title = title;
        self
    }

    pub fn description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }
}

impl Section {
    pub fn get_chapters(&self) -> anyhow::Result<Vec<Chapter>> {
        let query = format!(
            r#"
            PREFIX dc: <http://purl.org/dc/terms/>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            SELECT DISTINCT ?graph ?iri ?title ?content WHERE {{
                GRAPH ?graph {{
                    ?iri rdf:type <http://example.org/Chapter> ;
                         dc:title ?title ;
                         <http://example.org/schema/belongsTo> {} .
                    OPTIONAL {{ ?iri <http://example.org/schema/content> ?content }}
                }}
            }}
            "#,
            self.get_named_node().to_string(),
        );

        let results = self.get_model().get_store().query(&query)?;
        let mut chapters = Vec::new();

        if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                let solution = solution?;
                let chapter = Chapter {
                    element_ref: ElementRef::from_solution(
                        &self.get_model(),
                        &solution,
                    )?,
                    title:       solution.get("title").map(|s| s.to_string()),
                    content:     solution.get("content").map(|s| s.to_string()),
                };
                chapters.push(chapter);
            }
        }

        Ok(chapters)
    }
}
