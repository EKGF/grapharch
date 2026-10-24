use {
    grapharch::publication::validate,
    oxigraph::{io::RdfFormat, store::Store},
};

fn fixture() -> anyhow::Result<Store> {
    let store = Store::new()?;
    store.load_from_reader(
        RdfFormat::Turtle,
        include_bytes!("fixtures/publication/projects.ttl").as_slice(),
    )?;
    Ok(store)
}

#[test]
fn three_generic_projects_and_multiple_targets_validate() -> anyhow::Result<()>
{
    validate(&fixture()?)
}

#[test]
fn missing_target_scope_is_rejected() -> anyhow::Result<()> {
    let store = fixture()?;
    store.update("DELETE WHERE { <urn:example:spec> <https://ekgf.org/ontology/grapharch#scope> ?scope }")?;
    assert!(validate(&store).unwrap_err().to_string().contains("scope"));
    Ok(())
}

#[test]
fn ambiguous_target_profile_is_rejected() -> anyhow::Result<()> {
    let store = fixture()?;
    store.update("INSERT DATA { <urn:example:spec> <https://ekgf.org/ontology/grapharch#profile> <urn:example:reader-profile> }")?;
    assert!(
        validate(&store)
            .unwrap_err()
            .to_string()
            .contains("profile")
    );
    Ok(())
}

#[test]
fn unresolved_component_reference_is_rejected() -> anyhow::Result<()> {
    let store = fixture()?;
    store.update("INSERT DATA { <urn:example:introduction> <https://ekgf.org/ontology/grapharch#fragment> <urn:example:missing> }")?;
    assert!(
        validate(&store)
            .unwrap_err()
            .to_string()
            .contains("fragment")
    );
    Ok(())
}

#[test]
fn target_domain_interface_preserves_independent_scope_and_profile()
-> anyhow::Result<()> {
    use {
        grapharch::publication::{Renderer, Target},
        oxigraph::model::NamedNode,
    };
    let store = fixture()?;
    let spec = Target::read(&store, NamedNode::new("urn:example:spec")?)?;
    let reader = Target::read(&store, NamedNode::new("urn:example:reader")?)?;
    assert_eq!(spec.renderer, Renderer::Html);
    assert_eq!(reader.renderer, Renderer::Html);
    assert_ne!(spec.scope, reader.scope);
    assert_ne!(spec.profile, reader.profile);
    Ok(())
}

#[test]
fn vocabulary_and_shapes_are_valid_rdf() -> anyhow::Result<()> {
    for source in [
        include_str!("../docs/other/ontology/publication.ttl"),
        include_str!("../docs/other/ontology/publication.shacl.ttl"),
    ] {
        Store::new()?.load_from_reader(RdfFormat::Turtle, source.as_bytes())?;
    }
    Ok(())
}

#[test]
fn identical_assertions_in_overlapping_contributions_are_not_conflicts()
-> anyhow::Result<()> {
    let store = fixture()?;
    store.update("INSERT DATA { GRAPH <urn:overlap> { <urn:example:spec> <https://ekgf.org/ontology/grapharch#profile> <urn:example:spec-profile> } }")?;
    validate(&store)
}

#[test]
fn depth_with_string_datatype_is_rejected() -> anyhow::Result<()> {
    let store = fixture()?;
    store.update("DELETE WHERE { <urn:example:family> <https://ekgf.org/ontology/grapharch#maxDepth> ?v }; INSERT DATA { <urn:example:family> <https://ekgf.org/ontology/grapharch#maxDepth> \"2\" }")?;
    assert!(validate(&store).is_err());
    Ok(())
}

#[test]
fn stage_contributions_can_be_owned_by_a_documentor() -> anyhow::Result<()> {
    let store = fixture()?;
    store.update("DELETE WHERE { <urn:example:authored> <https://ekgf.org/ontology/grapharch#processor> ?old }; INSERT DATA { <urn:example:documentor> a <https://ekgf.org/ontology/grapharch#Documentor> . <urn:example:authored> <https://ekgf.org/ontology/grapharch#processor> <urn:example:documentor> }")?;
    validate(&store)
}

#[test]
fn a_fragment_requires_at_least_one_documentable_subject() -> anyhow::Result<()>
{
    let store = fixture()?;
    store.update("DELETE WHERE { <urn:example:intro-fragment> <https://ekgf.org/ontology/grapharch#subject> ?subject }")?;
    assert!(validate(&store).is_err());
    Ok(())
}

#[test]
fn placement_orders_require_integer_datatypes_not_numeric_strings()
-> anyhow::Result<()> {
    let store = fixture()?;
    store.update("DELETE WHERE { <urn:example:spec-intro> <https://ekgf.org/ontology/grapharch#order> ?order }; INSERT DATA { <urn:example:spec-intro> <https://ekgf.org/ontology/grapharch#order> \"1\" }")?;
    assert!(validate(&store).is_err());
    Ok(())
}
