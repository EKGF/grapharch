use {
    super::{Buildable, Element, element::ElementRef},
    anyhow::Result,
    oxrdf::Literal,
};

#[derive(Debug, Clone)]
pub struct Chapter {
    pub(super) element_ref: ElementRef,
    pub title:              Option<String>,
    pub content:            Option<String>,
}

impl Element for Chapter {
    type Builder = ChapterBuilder;

    fn builder(element_ref: ElementRef) -> anyhow::Result<Self::Builder> {
        Ok(ChapterBuilder::new(element_ref))
    }

    fn get_element_ref(&self) -> &ElementRef { &self.element_ref }
}

pub struct ChapterBuilder {
    pub(super) element_ref: ElementRef,
    pub title:              Option<String>,
    pub content:            Option<String>,
}

impl Buildable<Chapter> for ChapterBuilder {
    fn get_element_ref(&self) -> &ElementRef { &self.element_ref }

    fn new(element_ref: ElementRef) -> Self {
        Self { element_ref, title: None, content: None }
    }

    fn build(&mut self) -> Result<Chapter> {
        let chapter = Chapter {
            element_ref: self.element_ref.clone(),
            title:       self.title.clone(),
            content:     self.content.clone(),
        };

        self.insert_type("http://example.org/Chapter")?;

        if let Some(title) = &self.title {
            self.insert_object_literal(
                oxrdf::NamedNode::new("http://purl.org/dc/terms/title")?,
                Literal::new_simple_literal(title),
            )?;
        }

        if let Some(content) = &self.content {
            self.insert_object_literal(
                oxrdf::NamedNode::new("http://example.org/schema/content")?,
                Literal::new_simple_literal(content),
            )?;
        }

        Ok(chapter)
    }
}

impl ChapterBuilder {
    pub fn title(mut self, title: Option<String>) -> Self {
        self.title = title;
        self
    }

    pub fn content(mut self, content: Option<&str>) -> Self {
        self.content = content.map(|s| s.to_string());
        self
    }
}
