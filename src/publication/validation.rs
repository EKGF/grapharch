use {
    super::{RDF_TYPE, definitions::objects, term},
    anyhow::{Result, bail, ensure},
    oxigraph::store::Store,
    oxrdf::{NamedNode, NamedNodeRef, Term},
};

pub(crate) struct Field {
    pub(crate) name:      &'static str,
    pub(crate) required:  bool,
    pub(crate) multiple:  bool,
    pub(crate) reference: Option<&'static str>,
    pub(crate) literal:   bool,
}

impl Field {
    const fn one(name: &'static str, reference: Option<&'static str>) -> Self {
        Self {
            name,
            required: true,
            multiple: false,
            reference,
            literal: false,
        }
    }

    const fn many(name: &'static str, reference: Option<&'static str>) -> Self {
        Self {
            name,
            required: false,
            multiple: true,
            reference,
            literal: false,
        }
    }

    const fn some(name: &'static str) -> Self {
        Self {
            name,
            required: true,
            multiple: true,
            reference: None,
            literal: false,
        }
    }

    const fn text(name: &'static str) -> Self {
        Self {
            name,
            required: true,
            multiple: false,
            reference: None,
            literal: true,
        }
    }

    const fn optional_order() -> Self {
        Self {
            name:      "order",
            required:  false,
            multiple:  false,
            reference: None,
            literal:   true,
        }
    }

    pub(crate) fn check_literal(
        &self,
        value: &Term,
        subject: &NamedNode,
    ) -> Result<()> {
        let Term::Literal(literal) = value else {
            bail!("{subject} {} must be a literal", self.name)
        };
        if self.name == "order" {
            unsigned(value, "order")?;
        }
        if self.name == "destination" {
            ensure!(
                !literal.value().is_empty(),
                "{subject} empty destination"
            );
        }
        Ok(())
    }

    fn validate(&self, store: &Store, subject: &NamedNode) -> Result<()> {
        let mut count = 0;
        for object in objects(store, subject, self.name)? {
            let object = object?;
            count += 1;
            if self.literal {
                self.check_literal(&object, subject)?;
            } else {
                let Term::NamedNode(ref node) = object else {
                    bail!("{subject} {} must reference an IRI", self.name)
                };
                if let Some(class) = self.reference {
                    ensure!(
                        has_type(store, node, class)?,
                        "{subject} {} references undefined {class}: {node}",
                        self.name
                    );
                }
            }
        }
        ensure!(
            !self.required || count > 0,
            "{subject} missing {}",
            self.name
        );
        ensure!(
            self.multiple || count <= 1,
            "{subject} has conflicting {} values",
            self.name
        );
        Ok(())
    }
}

fn has_type(store: &Store, node: &NamedNode, class: &str) -> Result<bool> {
    if class == "Processor" &&
        (has_type(store, node, "Collector")? ||
            has_type(store, node, "Documentor")?)
    {
        return Ok(true);
    }
    Ok(store
        .quads_for_pattern(
            Some(node.as_ref().into()),
            Some(NamedNodeRef::new_unchecked(RDF_TYPE)),
            Some(term(class).as_ref().into()),
            None,
        )
        .next()
        .transpose()?
        .is_some())
}

/// Validate normalized definitions using the same constraints as the shipped
/// SHACL. Iterates graph assertions without hydrating the project or corpus
/// into memory.
pub fn validate(store: &Store) -> Result<()> {
    for (class, fields) in rules() {
        for quad in store.quads_for_pattern(
            None,
            Some(NamedNodeRef::new_unchecked(RDF_TYPE)),
            Some(term(class).as_ref().into()),
            None,
        ) {
            let subject = quad?.subject;
            let oxrdf::Subject::NamedNode(subject) = subject else {
                bail!("{class} requires a stable IRI, found {subject}")
            };
            for field in &fields {
                field.validate(store, &subject)?;
            }
            if class == "Target" {
                for quad in store.quads_for_pattern(
                    Some(subject.as_ref().into()),
                    Some(term("renderer").as_ref()),
                    None,
                    None,
                ) {
                    let Term::Literal(renderer) = quad?.object else {
                        unreachable!()
                    };
                    ensure!(
                        crate::publication::Renderer::parse(renderer.value())
                            .is_ok(),
                        "{subject} unsupported renderer {renderer}"
                    );
                }
            }
            if class == "Scope" {
                validate_depth(store, &subject)?;
            }
        }
    }
    Ok(())
}

fn validate_depth(store: &Store, subject: &NamedNode) -> Result<()> {
    let mut depths = objects(store, subject, "maxDepth")?;
    let depth = depths.next().transpose()?;
    ensure!(
        depths.next().transpose()?.is_none(),
        "{subject} conflicting maxDepth"
    );
    let traverses = store
        .quads_for_pattern(
            Some(subject.as_ref().into()),
            Some(term("traverse").as_ref()),
            None,
            None,
        )
        .next()
        .transpose()?
        .is_some();
    ensure!(
        !traverses || depth.is_some(),
        "{subject} traverse requires maxDepth"
    );
    if let Some(depth) = depth {
        unsigned(&depth, "scope maxDepth")?;
    }

    Ok(())
}

pub(crate) fn rules() -> impl Iterator<Item = (&'static str, Vec<Field>)> {
    // bounded: the fixed vocabulary schema, independent of corpus size.
    [
        ("Project", vec![
            Field::many("scope", Some("Scope")),
            Field::many("target", Some("Target")),
            Field::many("collector", Some("Collector")),
        ]),
        ("Collector", vec![
            Field::text("kind"),
            Field::text("source"),
        ]),
        ("Scope", vec![
            Field::many("include", None),
            Field::many("exclude", None),
            Field::many("compose", Some("Scope")),
            Field::many("traverse", None),
        ]),
        ("Target", vec![
            Field::one("scope", Some("Scope")),
            Field::one("profile", Some("Profile")),
            Field::text("renderer"),
            Field::text("destination"),
        ]),
        ("Profile", vec![Field::many(
            "placement",
            Some("Placement"),
        )]),
        ("Placement", vec![
            Field::one("component", Some("Component")),
            Field::text("order"),
        ]),
        ("Component", vec![Field::many(
            "fragment",
            Some("Fragment"),
        )]),
        ("Fragment", vec![
            Field::optional_order(),
            Field::some("subject"),
            Field::text("body"),
            Field::text("format"),
            Field::one("contribution", Some("Contribution")),
        ]),
        ("Contribution", vec![
            Field::one("processor", Some("Processor")),
            Field::one("project", Some("Project")),
            Field::text("source"),
            Field::text("revision"),
        ]),
    ]
    .into_iter()
}

pub(crate) fn unsigned(value: &Term, label: &str) -> Result<u32> {
    let Term::Literal(value) = value else {
        bail!("{label} must be an integer literal")
    };
    ensure!(
        value.datatype().as_str() == "http://www.w3.org/2001/XMLSchema#integer",
        "{label} requires an xsd:integer literal"
    );
    Ok(value.value().parse::<u32>()?)
}
