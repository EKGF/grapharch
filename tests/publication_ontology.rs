//! The vocabulary is the public contract for both authored and exported RDF.
use {
    anyhow::Result,
    oxigraph::{io::RdfFormat, sparql::QueryResults, store::Store},
    regex::Regex,
    std::{collections::BTreeSet, path::Path},
};

fn ontology() -> Result<Store> {
    let store = Store::new()?;
    store.load_from_reader(
        RdfFormat::Turtle,
        include_bytes!("../docs/other/ontology/publication.ttl").as_slice(),
    )?;
    Ok(store)
}

fn ask(store: &Store, pattern: &str) -> Result<bool> {
    let QueryResults::Boolean(value) = oxigraph::sparql::SparqlEvaluator::new().parse_query(format!(
        "PREFIX ga: <https://ekgf.org/ontology/grapharch#> \
         PREFIX owl: <http://www.w3.org/2002/07/owl#> \
         PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#> \
         PREFIX dct: <http://purl.org/dc/terms/> \
         PREFIX skos: <http://www.w3.org/2004/02/skos/core#> ASK {{ {pattern} }}"
    ).as_str())?.on_store(store).execute()? else { panic!("expected ASK result") };
    Ok(value)
}

#[test]
fn ontology_has_publication_metadata_and_human_definitions() -> Result<()> {
    let store = ontology()?;
    assert!(ask(
        &store,
        "<https://ekgf.org/ontology/grapharch> a owl:Ontology ; \
         owl:versionIRI ?version ; owl:versionInfo ?status ; dct:title ?title \
         ; dct:description ?description ; dct:license ?license ; dct:creator \
         ?creator ; dct:modified ?date ; rdfs:seeAlso ?shapes . \
         FILTER(isIRI(?version) && isIRI(?license) && lang(?description) = \
         'en')"
    )?);
    assert!(!ask(&store, "?term a ?type . VALUES ?type { owl:Class owl:ObjectProperty owl:DatatypeProperty owl:AnnotationProperty } FILTER(isIRI(?term) && STRSTARTS(STR(?term), STR(ga:))) FILTER NOT EXISTS { ?term rdfs:isDefinedBy <https://ekgf.org/ontology/grapharch> ; rdfs:label ?label ; skos:definition ?definition ; dct:description ?definition . FILTER(lang(?label) = 'en' && lang(?definition) = 'en' && STRLEN(STR(?definition)) > 40) }")?, "every public term needs an English definition and ontology membership");
    assert!(
        !ask(
            &store,
            "?p a ?type . VALUES ?type { owl:ObjectProperty \
             owl:DatatypeProperty } FILTER NOT EXISTS { ?p rdfs:domain \
             ?domain ; rdfs:range ?range }"
        )?,
        "properties need explicit domains and ranges"
    );
    assert!(!ask(
        &store,
        "?p a owl:ObjectProperty, owl:DatatypeProperty"
    )?);
    Ok(())
}

fn sources(path: &Path, output: &mut String) -> Result<()> {
    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            sources(&path, output)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push_str(&std::fs::read_to_string(path)?);
            output.push('\n');
        }
    }
    Ok(())
}

#[test]
fn implementation_terms_are_declared_in_the_public_vocabulary() -> Result<()> {
    let mut source = String::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/publication"),
        &mut source,
    )?;
    let names = Regex::new(
        r#"term\("([A-Za-z][A-Za-z0-9]*)"\)|(?:ga:|https://ekgf.org/ontology/grapharch#)([A-Za-z][A-Za-z0-9]*)"#,
    )?;
    let used: BTreeSet<_> = names
        .captures_iter(&source)
        .map(|capture| {
            capture
                .get(1)
                .or_else(|| capture.get(2))
                .unwrap()
                .as_str()
                .to_owned()
        })
        .collect();
    let store = ontology()?;
    let mut missing = Vec::new();
    for name in used {
        if !ask(
            &store,
            &format!(
                "ga:{name} a ?kind . VALUES ?kind {{ owl:Class \
                 owl:ObjectProperty owl:DatatypeProperty \
                 owl:AnnotationProperty }}"
            ),
        )? {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "undocumented runtime vocabulary: {missing:?}"
    );
    Ok(())
}

#[test]
fn embedded_turtle_examples_are_parseable() -> Result<()> {
    let store = ontology()?;
    let QueryResults::Solutions(examples) = oxigraph::sparql::SparqlEvaluator::new().parse_query(
        "SELECT ?term ?example WHERE { ?term <http://www.w3.org/2004/02/skos/core#example> ?example }",
    )?.on_store(&store).execute()? else { panic!("expected examples") };
    let mut count = 0;
    for example in examples {
        let example = example?;
        let oxrdf::Term::Literal(value) = &example["example"] else {
            panic!("example must be Turtle text")
        };
        Store::new()?
            .load_from_reader(RdfFormat::Turtle, value.value().as_bytes())?;
        count += 1;
    }
    assert!(
        count > 0,
        "the ontology must contain worked RDF examples"
    );
    Ok(())
}

#[tokio::test]
async fn collected_dataset_uses_declared_property_kinds() -> Result<()> {
    let (dataset, _) = grapharch::publication::showcases::dprod(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dprod"),
    )
    .await?;
    let store = ontology()?;
    let mut rows = dataset.select(
        "SELECT DISTINCT ?predicate (isLiteral(?value) AS ?literal) WHERE { GRAPH ?g { ?s ?predicate ?value } FILTER(STRSTARTS(STR(?predicate), 'https://ekgf.org/ontology/grapharch#')) }".into()
    ).await?;
    while let Some(row) = rows.next().await? {
        let oxrdf::Term::Literal(literal) = &row["literal"] else {
            panic!("expected boolean")
        };
        let kind = if literal.value() == "true" {
            "DatatypeProperty"
        } else {
            "ObjectProperty"
        };
        assert!(
            ask(
                &store,
                &format!("{} a owl:{kind}", row["predicate"])
            )?,
            "{} has an undeclared or incompatible property kind",
            row["predicate"]
        );
    }
    Ok(())
}

#[test]
fn retained_asset_references_do_not_classify_authored_fragments_as_plans()
-> Result<()> {
    let store = ontology()?;
    assert!(
        ask(
            &store,
            "ga:asset rdfs:domain rdfs:Resource ; rdfs:range ga:BinaryAsset"
        )?,
        "asset references are also emitted on authored fragments and \
         collected references"
    );
    assert!(!ask(&store, "ga:asset rdfs:domain ga:Plan")?);
    Ok(())
}
