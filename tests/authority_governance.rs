#[path = "support/definitions.rs"]
mod definitions;

use {
    anyhow::Result,
    grapharch::publication::{render::Publisher, showcases},
    std::path::Path,
};

#[tokio::test]
async fn canonical_governance_authority_renders_as_a_role_with_a_distinct_player()
-> Result<()> {
    let fixture =
        std::fs::read_to_string("examples/specification/project.ttl")?;
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let role = "urn:uuid:6c1408aa-039c-42f9-8dee-9c3f0009d61a";
    let organization = "urn:uuid:ee14e632-d8c2-43c1-95d8-72491ce7a846";
    let fixture = definitions::edit_quads(&fixture, |quads| {
        use oxrdf::{NamedNode, Term};
        for quad in quads {
            if quad.predicate.as_str() ==
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" &&
                quad.object ==
                    definitions::iri(
                        "https://ekgf.org/ontology/grapharch#Authority",
                    )?
            {
                quad.object = definitions::iri(
                    "https://ekgf.org/ontology/artifact-governance#Authority",
                )?;
            }
            if quad.predicate.as_str() ==
                "https://ekgf.org/ontology/grapharch#shortName"
            {
                quad.predicate=NamedNode::new("https://ekgf.org/ontology/artifact-governance#authorityAcronym")?;
            }
            if quad.predicate.as_str() == "http://purl.org/dc/terms/title" &&
                quad.object ==
                    definitions::literal(
                        "Enterprise Knowledge Graph Forum",
                    )
            {
                quad.predicate = NamedNode::new(
                    "http://www.w3.org/2000/01/rdf-schema#label",
                )?;
                quad.object =
                    Term::Literal(oxrdf::Literal::new_language_tagged_literal(
                        "Enterprise Knowledge Graph Forum",
                        "en",
                    )?);
            }
        }
        Ok(())
    })?;
    let fixture = format!("{fixture}\n<{role}> <https://www.omg.org/spec/Commons/RolesAndCompositions/isPlayedBy> <{organization}> .\n<{organization}> a <https://www.omg.org/spec/Commons/Organizations/Organization> .\n");
    std::fs::write(&path, &fixture)?;
    let (dataset, plans) = showcases::specification(Path::new(&path)).await?;
    std::fs::remove_file(&path)?;
    let output = tempfile::tempdir()?;
    showcases::publish(&Publisher::new(dataset), &plans, output.path()).await?;
    let html = std::fs::read_to_string(
        output.path().join("specification/index.html"),
    )?;
    assert!(html.contains("EKGF Editor&#39;s Draft"));
    assert!(html.contains("alt=\"Enterprise Knowledge Graph Forum\""));
    Ok(())
}

#[tokio::test]
async fn grapharch_authority_alias_is_rejected() -> Result<()> {
    let fixture =
        std::fs::read_to_string("examples/specification/project.ttl")?;
    let fixture = definitions::edit_quads(&fixture, |quads| {
        for quad in quads {
            if quad.predicate.as_str() ==
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" &&
                matches!(quad.object.to_string().as_str(),
                    "<https://ekgf.org/ontology/grapharch#Authority>" |
                    "<https://ekgf.org/ontology/artifact-governance#Authority>")
            {
                quad.object = definitions::iri(
                    "https://ekgf.org/ontology/grapharch#Authority",
                )?;
            }
        }
        Ok(())
    })?;
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    std::fs::write(&path, fixture)?;
    let (dataset, plans) = showcases::specification(&path).await?;
    let output = tempfile::tempdir()?;
    let failure =
        showcases::publish(&Publisher::new(dataset), &plans, output.path())
            .await
            .err()
            .expect("obsolete ga:Authority must be rejected");
    assert!(format!("{failure:#}").contains("Authority"));
    Ok(())
}

#[test]
fn publication_imports_authority_without_redeclaring_an_alias() -> Result<()> {
    use oxigraph::{io::RdfFormat, sparql::QueryResults, store::Store};
    let store = Store::new()?;
    store.load_from_reader(
        RdfFormat::Turtle,
        include_bytes!("../docs/other/ontology/publication.ttl").as_slice(),
    )?;
    let QueryResults::Boolean(imported) = oxigraph::sparql::SparqlEvaluator::new().parse_query("ASK { ?ontology <http://www.w3.org/2002/07/owl#imports> <https://ekgf.org/ontology/artifact-governance#> FILTER NOT EXISTS { <https://ekgf.org/ontology/grapharch#Authority> ?predicate ?object } }")?.on_store(&store).execute()? else { panic!("ASK required") };
    assert!(
        imported,
        "reuse the imported role directly without a Grapharch alias"
    );
    Ok(())
}
