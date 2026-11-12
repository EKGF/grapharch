use {
    crate::publication::{
        RDF_TYPE,
        dataset::identity,
        document::namespace_bindings::NamespaceBindings,
        term,
    },
    oxrdf::{GraphName, Literal, NamedNode, Quad, Term},
};

pub(super) fn q(
    subject: &NamedNode,
    field: &str,
    object: impl Into<Term>,
) -> Quad {
    Quad::new(
        subject.clone(),
        if field == "type" {
            NamedNode::new_unchecked(RDF_TYPE)
        } else {
            term(field)
        },
        object,
        GraphName::DefaultGraph,
    )
}
pub(super) fn placement(
    quads: &mut Vec<Quad>,
    target: &NamedNode,
    profile: &NamedNode,
    component: &NamedNode,
    title: &str,
    kind: &str,
    order: u32,
) {
    let placement = identity("ontology-placement", &[
        target.as_str(),
        component.as_str(),
    ]);
    quads.extend([
        q(profile, "placement", placement.clone()),
        q(&placement, "type", term("Placement")),
        q(&placement, "component", component.clone()),
        q(&placement, "order", Literal::from(order)),
        q(
            &placement,
            "sectionKind",
            Literal::new_simple_literal(kind),
        ),
        q(component, "type", term("Component")),
    ]);
    quads.push(Quad::new(
        component.clone(),
        NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
        Literal::new_simple_literal(title),
        GraphName::DefaultGraph,
    ));
}

/// Display bindings for one ontology specification: the well-known
/// vocabularies, the artifact-governance vocabulary its metadata uses, and the
/// ontology's own preferred namespace, which outranks both.
pub(super) fn prefixes(
    namespace: &str,
    prefix: &str,
) -> anyhow::Result<NamespaceBindings> {
    let mut bindings = NamespaceBindings::well_known().with_default(
        "https://ekgf.org/ontology/artifact-governance#",
        "artgov",
    );
    bindings.declare(namespace, prefix)?;
    Ok(bindings)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reused_commons_role_terms_have_readable_identifiers() {
        let prefixes = super::prefixes("urn:example:", "ex")
            .expect("valid declaration")
            .into_map();
        assert_eq!(
            prefixes
                .get("https://www.omg.org/spec/Commons/RolesAndCompositions/"),
            Some(&"cmns-rlcmp".to_owned())
        );
    }
}
