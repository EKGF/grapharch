//! The drawing state of one diagram and its rendering order.
use {
    crate::publication::document::class_diagram::{
        Diagram,
        Node,
        constraint_projection,
        layout::Layout,
        shorthand::Shorthand,
        verbalization::Verbalization,
    },
    std::collections::{BTreeMap, BTreeSet},
};

const HEADER: &str =
    "@startuml\n!pragma layout smetana\nallowmixing\nleft to right \
     direction\nhide empty members\nhide circle\nskinparam shadowing \
     false\nskinparam backgroundColor transparent\nskinparam defaultFontName \
     Arial\nskinparam defaultFontSize 14\nskinparam ArrowColor \
     #68758A\nskinparam nodesep 30\nskinparam ranksep 45\n";

/// A drawn diagram: its PlantUML source and its plain-language description.
pub(in super::super) struct Rendering {
    pub source:      String,
    pub description: String,
}
pub(in super::super) struct PlantUmlSource<'a> {
    pub(super) diagram:     &'a Diagram,
    pub(super) nodes:       BTreeMap<&'a str, &'a Node>, /* bounded: thirty-seven nodes. */
    pub(super) identifiers: BTreeMap<&'a str, usize>, /* bounded: thirty-seven nodes. */
    pub(super) shorthand:   Shorthand<'a>,
    pub(super) layout:      Layout<'a>,
    pub(super) visible:     BTreeSet<&'a str>, // bounded: thirty-seven nodes.
    pub(super) output:      String,
    pub(super) words:       Verbalization,
}
impl<'a> PlantUmlSource<'a> {
    pub fn new(diagram: &'a Diagram) -> Self {
        let nodes = Self::index(diagram);
        let identifiers = nodes
            .keys()
            .enumerate()
            .map(|(index, id)| (*id, index))
            .collect(); // bounded: thirty-seven nodes.
        let shorthand = Shorthand::new(diagram);
        let mut visible = shorthand.visible.clone();
        for (class, field) in constraint_projection::properties(diagram) {
            visible.insert(class.id.as_str());
            if field.object &&
                !field.prohibited() &&
                let Some(range) = field.range
            {
                visible.insert(range.id.as_str());
            }
        }
        Self {
            diagram,
            nodes,
            identifiers,
            shorthand,
            layout: Layout::new(diagram),
            visible,
            output: String::new(),
            words: Verbalization::default(),
        }
    }

    /// Every distinct node in deterministic identity order.
    pub fn index(diagram: &Diagram) -> BTreeMap<&str, &Node> {
        diagram
            .nodes()
            .map(|node| (node.id.as_str(), node))
            .collect() // bounded: at most thirty-seven nodes.
    }

    pub fn render(mut self) -> Rendering {
        self.output.push_str(
            &if self.layout.has_hierarchy() {
                HEADER.replacen(
                    "left to right direction",
                    "top to bottom direction",
                    1,
                )
            } else {
                HEADER.to_owned()
            },
        );
        let nodes: Vec<_> = self.nodes.values().copied().collect(); // bounded: thirty-seven nodes.
        for node in nodes {
            if self.visible.contains(node.id.as_str()) {
                self.node(node);
            }
        }
        self.placeholder();
        self.associations();
        self.restrictions();
        self.generalizations();
        self.remaining_facts();
        self.notes();
        self.output.push_str("@enduml\n");
        Rendering {
            description: self.words.describe(&self.diagram.focus),
            source:      self.output,
        }
    }

    pub(super) fn id(&self, node: &Node) -> usize {
        self.identifiers[node.id.as_str()]
    }
}
