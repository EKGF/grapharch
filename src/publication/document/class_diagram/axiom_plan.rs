//! The UML elements that state a class's anonymous OWL axioms.
//!
//! | Axiom | UML |
//! | --- | --- |
//! | `C ⊑ restriction` | restriction notation |
//! | `C ≡ restriction` | the same notation with `{defining}` |
//! | `C ⊑ A ⊓ …`, `C ≡ A ⊓ …` | a generalization to each named part; restriction parts as above |
//! | `C ≡ A ⊔ B` | a `{complete}` generalization set of `A` and `B` under `C` |
//! | `C ⊑ {a, b}`, `C ≡ {a, b}` | `«enumeration»` with the literals |
//! | `C ⊑ ¬ D` | a `{not D}` constraint note |
//! | anything else | an OCL-style constraint note |
//!
//! A neighbouring class shows only the restrictions it can draw as members.
use super::{
    Node,
    expression::{AxiomRelation, ClassAxiom, ClassExpression},
    restriction::{Restriction, Scope},
};

/// A generalization a class axiom implies.
pub(super) struct Generalization<'a> {
    pub child:    &'a Node,
    pub parent:   &'a Node,
    /// One of a `{complete}` generalization set: the parent is the union.
    pub complete: bool,
}
/// A plain-text constraint note attached to a class.
pub(super) struct Note<'a> {
    pub class: &'a Node,
    pub text:  String,
}
/// A class that is an enumeration of individuals or values.
pub(super) struct Enumeration<'a> {
    pub class:    &'a Node,
    pub literals: &'a [Node],
}
/// Every UML element that states the axioms of one scope.
#[derive(Default)]
pub(super) struct AxiomPlan<'a> {
    pub restrictions:    Vec<Restriction<'a>>,
    pub generalizations: Vec<Generalization<'a>>,
    pub notes:           Vec<Note<'a>>,
    pub enumerations:    Vec<Enumeration<'a>>,
}
impl<'a> AxiomPlan<'a> {
    pub fn new(axioms: &'a [ClassAxiom], scope: Scope) -> Self {
        let mut plan = Self::default();
        for axiom in axioms {
            plan.add(axiom, scope);
        }
        plan
    }

    fn add(&mut self, axiom: &'a ClassAxiom, scope: Scope) {
        let defining = axiom.relation == AxiomRelation::Equivalent;
        let class = &axiom.class;
        if !matches!(
            axiom.relation,
            AxiomRelation::Subclass | AxiomRelation::Equivalent
        ) {
            self.note(axiom, &axiom.expression, scope);
            return;
        }
        match &axiom.expression {
            ClassExpression::Intersection(parts) => {
                for part in parts {
                    match part {
                        ClassExpression::Named(parent) => {
                            if scope == Scope::Focus {
                                self.generalizations.push(Generalization {
                                    child: class,
                                    parent,
                                    complete: false,
                                });
                            }
                        },
                        _ => self.part(axiom, part, defining, scope),
                    }
                }
            },
            ClassExpression::Union(parts)
                if defining && parts.iter().all(|p| p.named().is_some()) =>
            {
                if scope == Scope::Focus {
                    self.generalizations.extend(
                        parts.iter().filter_map(ClassExpression::named).map(
                            |child| {
                                Generalization {
                                    child,
                                    parent: class,
                                    complete: true,
                                }
                            },
                        ),
                    );
                }
            },
            ClassExpression::OneOf(literals) if scope == Scope::Focus => {
                self.enumerations.push(Enumeration { class, literals });
            },
            expression => self.part(axiom, expression, defining, scope),
        }
    }

    /// A restriction, or the note stating any other part of an axiom.
    fn part(
        &mut self,
        axiom: &'a ClassAxiom,
        expression: &'a ClassExpression,
        defining: bool,
        scope: Scope,
    ) {
        if let ClassExpression::Restriction(restriction) = expression &&
            let Some(drawn) = Restriction::from_expression(
                &axiom.class,
                restriction,
                defining,
            )
        {
            self.restrictions.extend(drawn);
        } else {
            self.note(axiom, expression, scope);
        }
    }

    fn note(
        &mut self,
        axiom: &'a ClassAxiom,
        expression: &'a ClassExpression,
        scope: Scope,
    ) {
        if scope == Scope::Focus {
            self.notes.push(Note {
                class: &axiom.class,
                text:  axiom.note(expression),
            });
        }
    }
}
