use {anyhow::Result, std::path::Path};

pub async fn configure(
    family: &Path,
    family_iri: &str,
    term_namespace: &str,
) -> Result<tempfile::TempDir> {
    let sources = tempfile::tempdir()?;
    // bounded: explicit showcase definition members, never a live dataset scan.
    let mut members = std::collections::BTreeSet::new();
    let definitions = tokio::fs::read(family.join("project.ttl")).await?;
    for quad in oxrdfio::RdfParser::from_format(oxrdfio::RdfFormat::Turtle)
        .for_slice(&definitions)
    {
        let quad = quad?;
        if quad.predicate.as_str() ==
            "https://ekgf.org/ontology/artifact-governance#inFamily" &&
            quad.object.to_string() == format!("<{family_iri}>")
        {
            members.insert(quad.subject.to_string());
        }
    }
    assert!(!members.is_empty());
    for (index, member) in members.iter().enumerate() {
        let mut body = format!(
            "@prefix owl: <http://www.w3.org/2002/07/owl#> .\n\
             @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
             @prefix dct: <http://purl.org/dc/terms/> .\n\
             {member} a owl:Ontology ; rdfs:label \"Synthetic offline ontology {index}\"@en ; \
             dct:description \"Gallery fixture, not copied upstream content.\"@en .\n"
        );
        if member == &format!("<{term_namespace}>") {
            body.push_str(&format!(
                "<{term_namespace}Fixture> a owl:Class ; rdfs:label \
                 \"Synthetic fixture\"@en ; dct:description \"An offline test \
                 resource.\"@en ; rdfs:isDefinedBy {member} .\n"
            ));
        }
        tokio::fs::write(
            sources.path().join(format!("ontology-{index}.ttl")),
            body,
        )
        .await?;
    }
    let repository = tempfile::tempdir()?;
    let (url, commit) =
        super::family_repository::bare_copy(sources.path(), repository.path())?;
    // Replace the RDF documents with the local Git transport; the explicit
    // definitions, project and targets stay as configured.
    let configuration = family.join("showcase.ttl");
    let ga = "https://ekgf.org/ontology/grapharch#";
    let mut showcase = None;
    let mut quads = Vec::new(); // bounded: one small showcase configuration.
    for quad in oxrdfio::RdfParser::from_format(oxrdfio::RdfFormat::Turtle)
        .for_slice(&tokio::fs::read(&configuration).await?)
    {
        let quad = quad?;
        if quad.object.to_string() == format!("<{ga}Showcase>") {
            showcase = Some(quad.subject.clone());
        }
        if ![format!("{ga}rdfDocument"), format!("{ga}format")]
            .contains(&quad.predicate.as_str().to_owned())
        {
            quads.push(quad);
        }
    }
    let showcase = showcase.expect("configured showcase");
    for (setting, value) in [("gitRepository", url), ("gitRef", commit)] {
        quads.push(oxrdf::Quad::new(
            showcase.clone(),
            oxrdf::NamedNode::new(format!("{ga}{setting}"))?,
            oxrdf::Literal::new_simple_literal(value),
            oxrdf::GraphName::DefaultGraph,
        ));
    }
    let mut serializer =
        oxrdfio::RdfSerializer::from_format(oxrdfio::RdfFormat::Turtle)
            .for_writer(Vec::new());
    for quad in &quads {
        serializer.serialize_quad(quad)?;
    }
    tokio::fs::write(&configuration, serializer.finish()?).await?;
    Ok(repository)
}
