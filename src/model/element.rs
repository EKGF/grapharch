use {
    crate::model::Model,
    anyhow::Result,
    oxigraph::{model::NamedNode, sparql::QuerySolution},
    oxrdf::{GraphName, Literal, Quad},
    std::sync::Arc,
    uuid::Uuid,
};

/// A reference to an element in the model.
///
/// This is used to reference an element in the model without
/// having to know the exact details of the element.
///
/// ElementRefs point to the specific Documentation Model,
/// a specific Named Graph in that model, and a specific Named Node
/// in that graph.
#[derive(Debug, Clone)]
pub struct ElementRef {
    pub(super) named_node: NamedNode,
    pub(super) graph_name: GraphName,
    pub(super) model:      Arc<Model>,
}

impl ElementRef {
    pub(super) fn get_named_node(&self) -> &NamedNode { &self.named_node }

    pub(super) fn get_graph_name(&self) -> &GraphName { &self.graph_name }

    pub(super) fn get_model(&self) -> &Arc<Model> { &self.model }

    /// Create an ElementRef from a QuerySolution assuming that the solution
    /// contains an "iri" and a "graph" variable.
    pub(super) fn from_solution(
        model: &Arc<Model>,
        solution: &QuerySolution,
    ) -> Result<Self> {
        let iri = solution
            .get("iri")
            .ok_or_else(|| anyhow::anyhow!("Missing 'iri' in solution"))?
            .to_string();

        let graph = solution
            .get("graph")
            .ok_or_else(|| anyhow::anyhow!("Missing 'graph' in solution"))?
            .to_string();

        let named_node = NamedNode::new(iri)?;
        let graph_name = GraphName::NamedNode(NamedNode::new(graph)?);

        Ok(Self { named_node, graph_name, model: model.clone() })
    }
}

pub trait Buildable<T> {
    /// Create a new builder for the element.
    fn new(element_ref: ElementRef) -> Self;

    fn get_element_ref(&self) -> &ElementRef;

    fn get_model(&self) -> &Arc<Model> { self.get_element_ref().get_model() }

    fn get_named_node(&self) -> &NamedNode {
        self.get_element_ref().get_named_node()
    }

    fn get_graph_name(&self) -> &GraphName {
        self.get_element_ref().get_graph_name()
    }

    /// Build the element and add it to the model.
    fn build(&mut self) -> Result<T>;

    fn insert(&self, quad: &Quad) -> Result<()> {
        self.get_model().insert(quad)
    }

    fn insert_type(&self, iri: &str) -> Result<()> {
        let quad = Quad::new(
            self.get_named_node().clone(),
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
            NamedNode::new(iri)?,
            self.get_graph_name().clone(),
        );
        self.insert(&quad)
    }

    fn insert_object_literal(
        &self,
        predicate: NamedNode,
        object: Literal,
    ) -> Result<()> {
        let quad = Quad::new(
            self.get_named_node().clone(),
            predicate,
            object,
            self.get_graph_name().clone(),
        );
        self.insert(&quad)
    }
}

pub trait Element: Sized {
    type Builder: Buildable<Self>;

    fn get_element_ref(&self) -> &ElementRef;

    fn get_named_node(&self) -> &NamedNode {
        self.get_element_ref().get_named_node()
    }

    fn get_model(&self) -> &Arc<Model> { self.get_element_ref().get_model() }

    fn get_graph_name(&self) -> &GraphName {
        self.get_element_ref().get_graph_name()
    }

    fn builder(element_ref: ElementRef) -> anyhow::Result<Self::Builder>;

    fn builder_for_node_in_model(
        named_node: NamedNode,
        model: &Arc<Model>,
    ) -> anyhow::Result<Self::Builder> {
        Self::builder(ElementRef {
            named_node,
            graph_name: GraphName::DefaultGraph,
            model: model.clone(),
        })
    }

    fn builder_in_model<E: Element>(
        model: &Arc<Model>,
    ) -> anyhow::Result<E::Builder> {
        let named_node = Self::new_named_node()?;
        E::builder(ElementRef {
            named_node,
            graph_name: GraphName::DefaultGraph,
            model: model.clone(),
        })
    }

    fn new_named_node() -> Result<NamedNode> {
        let uuid = Uuid::new_v4();
        let uri = format!("urn:uuid:{}", uuid);
        Ok(NamedNode::new(uri)?)
    }
}
