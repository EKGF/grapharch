//! The closed set of resource roles a class diagram distinguishes.
use {
    serde::{Deserialize, Serialize},
    std::fmt,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NodeKind {
    Class,
    Resource,
    #[serde(rename = "RDF property")]
    RdfProperty,
    #[serde(rename = "Object property")]
    ObjectProperty,
    #[serde(rename = "Datatype property")]
    DatatypeProperty,
    #[serde(rename = "Annotation property")]
    AnnotationProperty,
    #[serde(rename = "OWL restriction")]
    OwlRestriction,
    #[serde(rename = "OWL expression")]
    OwlExpression,
    #[serde(rename = "OWL union")]
    OwlUnion,
    #[serde(rename = "Literal value")]
    LiteralValue,
}
impl NodeKind {
    /// A named class or an otherwise untyped resource used as one.
    pub fn is_class_like(self) -> bool {
        matches!(self, Self::Class | Self::Resource)
    }

    pub fn is_property(self) -> bool {
        matches!(
            self,
            Self::RdfProperty |
                Self::ObjectProperty |
                Self::DatatypeProperty |
                Self::AnnotationProperty
        )
    }

    /// Anonymous OWL syntax drawn as its own box.
    pub fn is_expression(self) -> bool {
        matches!(self, Self::OwlExpression | Self::OwlUnion)
    }

    fn caption(self) -> &'static str {
        match self {
            Self::Class => "Class",
            Self::Resource => "Resource",
            Self::RdfProperty => "RDF property",
            Self::ObjectProperty => "Object property",
            Self::DatatypeProperty => "Datatype property",
            Self::AnnotationProperty => "Annotation property",
            Self::OwlRestriction => "OWL restriction",
            Self::OwlExpression => "OWL expression",
            Self::OwlUnion => "OWL union",
            Self::LiteralValue => "Literal value",
        }
    }
}
impl fmt::Display for NodeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.caption())
    }
}
