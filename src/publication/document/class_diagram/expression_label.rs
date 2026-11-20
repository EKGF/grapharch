//! Name each explicit anonymous expression box after its OWL form.
use {
    super::{Diagram, NodeKind},
    std::collections::BTreeMap,
};
const OWL: &str = "http://www.w3.org/2002/07/owl#";

/// The syntactic form of an anonymous class expression.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    Restriction,
    Union,
    Intersection,
    Complement,
    Enumeration,
}
impl Form {
    fn of(predicate: &str) -> Option<Self> {
        Some(match predicate.strip_prefix(OWL)? {
            "onProperty" => Self::Restriction,
            "unionOf" => Self::Union,
            "intersectionOf" => Self::Intersection,
            "complementOf" => Self::Complement,
            "oneOf" => Self::Enumeration,
            _ => return None,
        })
    }

    fn noun(self) -> &'static str {
        match self {
            Self::Restriction => "restriction",
            Self::Union => "union",
            Self::Intersection => "intersection",
            Self::Complement => "complement",
            Self::Enumeration => "enumeration",
        }
    }
}

/// Replace the generic noun of numbered captions such as "Anonymous OWL
/// expression 2" with the form, keeping the ordinal; mixed or unknown
/// forms stay "expression".
pub(super) fn apply(diagram: &mut Diagram) {
    let mut forms = BTreeMap::<String, Option<Form>>::new(); // bounded: thirty-seven nodes.
    for edge in &diagram.edges {
        if edge.from.kind != NodeKind::OwlExpression {
            continue;
        }
        let Some(form) = edge.predicate.as_deref().and_then(Form::of) else {
            continue;
        };
        forms
            .entry(edge.from.id.clone())
            .and_modify(|known| {
                if *known != Some(form) {
                    *known = None;
                }
            })
            .or_insert(Some(form));
    }
    for node in diagram.nodes_mut() {
        if let Some(Some(form)) = forms.get(&node.id) &&
            let Some(ordinal) =
                node.label.strip_prefix("Anonymous OWL expression ")
        {
            node.label = format!("Anonymous OWL {} {ordinal}", form.noun());
        }
    }
}
