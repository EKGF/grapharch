//! The vocabulary is the public contract for both authored and exported RDF.
#[path = "support/family_repository.rs"]
mod family_repository;
#[path = "support/revision_fixture.rs"]
mod revision_fixture;

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
    for catalogue in [
        include_bytes!("../docs/other/ontology/renderers.ttl").as_slice(),
        include_bytes!("../docs/other/ontology/collectors.ttl").as_slice(),
        include_bytes!("../docs/other/ontology/documentors.ttl").as_slice(),
        include_bytes!("../docs/other/ontology/showcase-runners.ttl")
            .as_slice(),
    ] {
        store.load_from_reader(RdfFormat::Turtle, catalogue)?;
    }
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
fn ontology_version_tracks_the_application_release() -> Result<()> {
    let release = env!("CARGO_PKG_VERSION");
    assert!(ask(&ontology()?, &format!(
        "<https://ekgf.org/ontology/grapharch> owl:versionIRI <https://ekgf.org/ontology/grapharch/{release}> ; owl:versionInfo ?status . FILTER(STRSTARTS(STR(?status), '{release} '))"
    ))?, "ontology version metadata must track Cargo.toml, including its draft status");
    Ok(())
}

#[test]
fn publishing_authorities_reuse_the_imported_governance_class_directly()
-> Result<()> {
    let store = ontology()?;
    assert!(
        !ask(&store, "ga:Authority ?predicate ?object")?,
        "GraphArch must not declare a duplicate Authority class or \
         equivalence alias"
    );
    assert!(ask(&store, "<https://ekgf.org/ontology/grapharch> owl:imports <https://ekgf.org/ontology/artifact-governance#> . ga:publisher rdfs:range <https://ekgf.org/ontology/artifact-governance#Authority> . VALUES ?property { ga:shortName ga:accentColor ga:logo } ?property rdfs:domain <https://ekgf.org/ontology/artifact-governance#Authority>")?);
    assert!(!ask(&store, "VALUES ?property { ga:shortName ga:accentColor ga:logo } FILTER NOT EXISTS { ?property rdfs:domain <https://ekgf.org/ontology/artifact-governance#Authority> }")?, "every authority-specific property must use the canonical domain");
    assert!(
        !ask(&store, "?subject ?predicate ga:Authority")?,
        "ontology properties must refer directly to the imported class"
    );
    store.load_from_reader(
        RdfFormat::Turtle,
        include_bytes!("../docs/other/ontology/publication.shacl.ttl")
            .as_slice(),
    )?;
    assert!(ask(&store, "ga:AuthorityShape <http://www.w3.org/ns/shacl#targetClass> <https://ekgf.org/ontology/artifact-governance#Authority>")?);
    assert!(ask(&store, "?shape <http://www.w3.org/ns/shacl#path> ga:publisher ; <http://www.w3.org/ns/shacl#class> <https://ekgf.org/ontology/artifact-governance#Authority>")?);
    assert!(
        !ask(&store, "?subject ?predicate ga:Authority")?,
        "SHACL must not accept the removed class alias"
    );
    Ok(())
}

#[test]
fn git_revision_hint_has_public_metadata_and_collector_constraints()
-> Result<()> {
    let store = ontology()?;
    assert!(ask(&store, "ga:gitRef a owl:DatatypeProperty ; rdfs:isDefinedBy <https://ekgf.org/ontology/grapharch> ; rdfs:domain ga:CollectorUse ; rdfs:range <http://www.w3.org/2001/XMLSchema#string> ; rdfs:label ?label ; skos:definition ?definition ; dct:description ?definition ; skos:example ?example . FILTER(lang(?label) = 'en' && lang(?definition) = 'en' && STRLEN(STR(?definition)) > 40 && CONTAINS(STR(?example), 'HEAD'))")?, "the Git revision hint needs a complete source-owned vocabulary definition");
    store.load_from_reader(
        RdfFormat::Turtle,
        include_bytes!("../docs/other/ontology/publication.shacl.ttl")
            .as_slice(),
    )?;
    assert!(ask(&store, "ga:CollectorUseShape <http://www.w3.org/ns/shacl#property> ?shape . ?shape <http://www.w3.org/ns/shacl#path> ga:gitRef ; <http://www.w3.org/ns/shacl#datatype> <http://www.w3.org/2001/XMLSchema#string> ; <http://www.w3.org/ns/shacl#minLength> 1 ; <http://www.w3.org/ns/shacl#maxCount> 1")?, "CollectorShape must constrain optional Git revision hints");
    assert!(ask(&store, "ga:CollectorUseShape <http://www.w3.org/ns/shacl#or> ?choices . ?choices <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>*/<http://www.w3.org/1999/02/22-rdf-syntax-ns#first> ?choice . ?choice <http://www.w3.org/ns/shacl#property> ?ref, ?path . ?ref <http://www.w3.org/ns/shacl#path> ga:gitRef ; <http://www.w3.org/ns/shacl#minCount> 1 . ?path <http://www.w3.org/ns/shacl#path> ga:sourcePath ; <http://www.w3.org/ns/shacl#minCount> 1")?, "configured Git RDF files require a revision and source path");
    Ok(())
}

#[test]
fn fragment_contributing_collector_uses_require_a_scope_inclusion() -> Result<()>
{
    let store = ontology()?;
    store.load_from_reader(
        RdfFormat::Turtle,
        include_bytes!("../docs/other/ontology/publication.shacl.ttl")
            .as_slice(),
    )?;
    let choice = "ga:IncludedInFragmentShape <http://www.w3.org/ns/shacl#targetClass> ga:CollectorUse ; <http://www.w3.org/ns/shacl#xone> ?choices . ?choices <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>*/<http://www.w3.org/1999/02/22-rdf-syntax-ns#first> ?choice . ?choice <http://www.w3.org/ns/shacl#property> ?fragment, ?inclusion . ?fragment <http://www.w3.org/ns/shacl#path> ga:fragment ; <http://www.w3.org/ns/shacl#maxCount> ?fragments . ?inclusion <http://www.w3.org/ns/shacl#path> ga:includedIn";
    assert!(
        ask(&store, &format!("{choice} ; <http://www.w3.org/ns/shacl#minCount> 1 . ?fragment <http://www.w3.org/ns/shacl#minCount> 1"))?,
        "a use naming its fragment must name a scope including it"
    );
    assert!(
        ask(
            &store,
            &format!(
                "{choice} ; <http://www.w3.org/ns/shacl#maxCount> 0 . \
                 FILTER(?fragments = 0)"
            )
        )?,
        "a use without a fragment cannot claim a scope inclusion"
    );
    Ok(())
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
                 owl:AnnotationProperty ga:Renderer ga:Collector \
                 ga:Documentor ga:ShowcaseRunner ga:PdfStandard }}"
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
    let fixture = revision_fixture::RevisionFixture::new(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dprod"),
    )?;
    let (dataset, _) = fixture
        .collect(grapharch::publication::revision_build::TargetSelection::All)
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

#[test]
fn heading_anchor_presentation_reuses_one_canonical_term_definition()
-> Result<()> {
    let store = ontology()?;
    for predicate in ["rdfs:label", "skos:definition", "dct:description"] {
        assert!(
            !ask(
                &store,
                &format!(
                    "ga:headingAnchor {predicate} ?first, ?second . \
                     FILTER(lang(?first) = 'en' && lang(?second) = 'en' && \
                     ?first != ?second)"
                )
            )?,
            "headingAnchor must have one canonical English {predicate}"
        );
    }
    Ok(())
}
