//! Theme-driven fills and font of a retained class diagram in a PDF.
//!
//! The retained engine SVG keeps the layout engine's colours and font. Each
//! entity box is recoloured by its resource role, and its text set in the
//! theme's diagram font, when the Typst theme dictionary is in scope. No
//! retained SVG text is evaluated as Typst source.
use {
    crate::publication::document::class_diagram::{
        Node,
        NodeKind,
        Projection,
        svg,
    },
    anyhow::{Result, ensure},
    std::{collections::BTreeMap, ops::Range},
};

/// The presentation role of one diagram box, which selects its fill token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    Focus,
    Class,
    ImportedClass,
    Expression,
    ObjectProperty,
    DatatypeProperty,
    Property,
    Datatype,
    Literal,
}

impl Role {
    pub fn of(node: &Node, focus: &Node) -> Self {
        if node.id == focus.id {
            return Self::Focus;
        }
        match node.kind {
            NodeKind::ObjectProperty => Self::ObjectProperty,
            NodeKind::DatatypeProperty => Self::DatatypeProperty,
            NodeKind::AnnotationProperty | NodeKind::RdfProperty => {
                Self::Property
            },
            NodeKind::OwlRestriction |
            NodeKind::OwlExpression |
            NodeKind::OwlUnion => Self::Expression,
            NodeKind::LiteralValue => Self::Literal,
            NodeKind::Class | NodeKind::Resource if node.datatype => {
                Self::Datatype
            },
            NodeKind::Class | NodeKind::Resource if node.imported => {
                Self::ImportedClass
            },
            NodeKind::Class | NodeKind::Resource => Self::Class,
        }
    }

    /// The `ga-theme` dictionary key of the role's fill.
    pub fn key(self) -> &'static str {
        match self {
            Self::Focus => "diagram_focus_fill",
            Self::Class => "diagram_class_fill",
            Self::ImportedClass => "diagram_imported_class_fill",
            Self::Expression => "diagram_expression_fill",
            Self::ObjectProperty => "diagram_object_property_fill",
            Self::DatatypeProperty => "diagram_datatype_property_fill",
            Self::Property => "diagram_property_fill",
            Self::Datatype => "diagram_datatype_fill",
            Self::Literal => "diagram_literal_fill",
        }
    }
}

/// A themed SVG: the retained text with attribute values replaced by
/// theme dictionary lookups.
pub(super) struct ThemedSvg {
    // bounded: one fill per diagram box plus the font attributes of a
    // validated 256 KiB SVG.
    replacements: Vec<(Range<usize>, String)>,
}

impl ThemedSvg {
    pub fn new(projection: &Projection, svg: &str) -> Result<Self> {
        let nodes: BTreeMap<String, &Node> = svg::entities(&projection.diagram); // bounded: thirty-seven nodes.
        let document = roxmltree::Document::parse(svg)?;
        let mut replacements = Vec::new();
        for element in document.descendants().filter(|n| n.is_element()) {
            if let Some(attribute) = element
                .attributes()
                .find(|attribute| attribute.name() == "font-family")
            {
                replacements.push((
                    attribute.range_value(),
                    "ga-theme.diagram_font".to_owned(),
                ));
            }
            if element.attribute("class") != Some("entity") {
                continue;
            }
            let Some(node) = element
                .attribute("data-qualified-name")
                .and_then(|name| nodes.get(name))
            else {
                continue;
            };
            let role = Role::of(node, &projection.diagram.focus);
            if let Some(attribute) = element
                .descendants()
                .filter(|shape| {
                    matches!(
                        shape.tag_name().name(),
                        "rect" | "polygon" | "path" | "ellipse"
                    )
                })
                .find_map(|shape| {
                    shape
                        .attributes()
                        .find(|attribute| attribute.name() == "fill")
                })
                .filter(|attribute| attribute.value() != "none")
            {
                replacements.push((
                    attribute.range_value(),
                    format!("ga-theme.{}.to-hex()", role.key()),
                ));
            }
        }
        replacements.sort_by_key(|(range, _)| range.start);
        ensure!(
            replacements
                .windows(2)
                .all(|pair| pair[0].0.end <= pair[1].0.start),
            "overlapping class diagram theme attributes"
        );
        Ok(Self { replacements })
    }

    /// A Typst string expression joining literal SVG text and theme values.
    pub fn typst(&self, svg: &str) -> String {
        let quote = super::super::markup::quote;
        let mut parts = Vec::with_capacity(self.replacements.len() * 2 + 1);
        let mut cursor = 0;
        for (range, value) in &self.replacements {
            parts.push(quote(&svg[cursor..range.start]));
            parts.push(value.clone());
            cursor = range.end;
        }
        parts.push(quote(&svg[cursor..]));
        format!("({},).join()", parts.join(", "))
    }
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
