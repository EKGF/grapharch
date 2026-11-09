use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        term,
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::{NamedNode, Term},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

async fn document(
    sources: &[&str],
    language: &str,
) -> Result<(String, Vec<String>)> {
    let root = tempfile::tempdir()?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    for (index, source) in sources.iter().enumerate() {
        let path = root.path().join(format!("source-{index}.ttl"));
        tokio::fs::write(&path, format!("@prefix sh:<http://www.w3.org/ns/shacl#> . @prefix dct:<http://purl.org/dc/terms/> . @prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> . {source}")).await?;
        RdfFile::new(
            "urn:test:project",
            &format!("urn:test:collector:{index}"),
            path,
        )?
        .collect(&writer)
        .await?;
    }
    let job = AnnotatedTermDocumentor::new(
        dataset.clone(),
        dataset.snapshot_for("urn:test:project").await?,
        "urn:test:project",
        NamedNode::new("urn:test:resource")?,
        language,
        Arc::new(BTreeMap::new()),
        Arc::new(HashSet::new()),
    )?
    .with_source_annotations();
    job.document(&writer).await?;
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:test:project").await?,
    );
    let body = view.text(&job.fragment_id(), "body").await?;
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?code WHERE {{ GRAPH ?g {{ ?diagnostic {} ?code \
             }} }} ORDER BY ?code",
            term("code")
        ))
        .await?;
    let mut codes = Vec::new();
    while let Some(row) = rows.next().await? {
        let Term::Literal(code) = &row["code"] else {
            panic!("literal diagnostic code")
        };
        codes.push(code.value().into());
    }
    drop(writer);
    worker.await??;
    Ok((body, codes))
}

#[tokio::test]
async fn shacl_names_and_descriptions_are_source_annotations() -> Result<()> {
    let (body, codes) = document(
        &[
            "<urn:test:resource> a sh:PropertyShape ; sh:name \"profile\" ; \
             sh:description \"Identifies the ODRL profile required by this \
             shape.\"@en .",
        ],
        "en",
    )
    .await?;
    assert!(
        codes.is_empty(),
        "SHACL supplied both annotations: {codes:?}"
    );
    assert!(
        body.contains("http://www.w3.org/ns/shacl#name") &&
            body.contains("profile")
    );
    assert!(!body.contains("**Label:**"));
    assert!(
        body.contains("Identifies the ODRL profile required by this shape.")
    );
    assert!(!body.contains("[!WARNING]"));
    Ok(())
}

#[tokio::test]
async fn missing_annotations_have_one_precise_language_aware_warning()
-> Result<()> {
    let (body, codes) =
        document(&["<urn:test:resource> a sh:NodeShape ."], "en").await?;
    assert_eq!(codes, [
        "missing-source-definition",
        "missing-source-label"
    ]);
    assert_eq!(body.matches("[!WARNING]").count(), 1);
    assert!(body.contains(
        "No English or language-neutral label or description found in the \
         collected sources."
    ));
    assert!(!body.contains("This source declares"));
    let (body, _) =
        document(&["<urn:test:resource> a sh:NodeShape ."], "nl").await?;
    assert!(!body.contains("English"));
    assert!(body.contains("'nl' or language-neutral"));
    Ok(())
}

#[tokio::test]
async fn annotations_from_selected_sources_are_combined_without_language_false_positives()
-> Result<()> {
    let (body, codes) = document(
        &[
            "<urn:test:resource> a sh:NodeShape ; sh:name \"Naam\"@nl ; \
             sh:description \"Uitleg\"@nl .",
            "<urn:test:resource> rdfs:label \"Name\"@en ; dct:description \
             \"Explanation\" .",
        ],
        "en",
    )
    .await?;
    assert!(
        codes.is_empty(),
        "selected sources supply English/neutral annotations"
    );
    assert!(
        body.contains("http://www.w3.org/2000/01/rdf-schema#label") &&
            body.contains("Name")
    );
    assert!(body.contains("Explanation"));
    assert!(
        body.contains("http://www.w3.org/ns/shacl#name") &&
            body.contains("Naam")
    );
    assert!(
        !body.contains("Uitleg"),
        "unselected narrative language remains excluded"
    );
    let (_, codes) = document(
        &[
            "<urn:test:resource> a sh:NodeShape ; sh:name \"Naam\"@nl ; \
             sh:description \"Uitleg\"@nl .",
        ],
        "en",
    )
    .await?;
    assert_eq!(
        codes.len(),
        2,
        "another language must not suppress English finding"
    );
    Ok(())
}

#[tokio::test]
async fn title_is_used_when_source_has_no_label_without_replacing_explicit_labels()
-> Result<()> {
    let (body, codes) = document(
        &[
            "<urn:test:resource> dct:title \"DPROD Contracts profile\"@en ; \
             dct:description \"An application profile.\"@en .",
        ],
        "en",
    )
    .await?;
    assert!(codes.is_empty());
    assert!(
        body.contains("http://purl.org/dc/terms/title") &&
            body.contains("DPROD Contracts profile")
    );
    let (body, _) = document(
        &[
            "<urn:test:resource> rdfs:label \"Profile\"@en ; dct:title \"Long \
             publication title\"@en ; dct:description \"Description\" .",
        ],
        "en",
    )
    .await?;
    assert!(
        body.contains("http://www.w3.org/2000/01/rdf-schema#label") &&
            body.contains("Profile")
    );
    assert!(body.contains("Long publication title"));
    Ok(())
}

#[tokio::test]
async fn dublin_core_namespace_aliases_and_skos_preferred_labels_are_recognised()
-> Result<()> {
    let (body, codes) = document(&["@prefix dcterms:<http://purl.org/dc/terms/> . @prefix skos:<http://www.w3.org/2004/02/skos/core#> . <urn:test:resource> rdfs:label \"Other name\"@en ; skos:prefLabel \"Preferred name\"@en ; dcterms:title \"Publication title\"@en ; dcterms:description \"English explanation\"@en ."], "en").await?;
    assert!(codes.is_empty());
    assert!(
        body.contains("http://www.w3.org/2004/02/skos/core#prefLabel") &&
            body.contains("Preferred name")
    );
    assert!(body.contains("Other name") && body.contains("Publication title"));
    let (body, codes) = document(
        &["@prefix dc:<http://purl.org/dc/elements/1.1/> . \
           <urn:test:resource> dc:title \"Legacy title\" ; dc:description \
           \"Legacy description\" ."],
        "en",
    )
    .await?;
    assert!(codes.is_empty());
    assert!(
        body.contains("http://purl.org/dc/elements/1.1/title") &&
            body.contains("Legacy title")
    );
    assert!(body.contains("Legacy description"));
    Ok(())
}

#[tokio::test]
async fn shacl_constraints_paths_lists_messages_and_sparql_are_documented_without_execution()
-> Result<()> {
    let (body, codes) = document(&[r#"
@prefix xsd:<http://www.w3.org/2001/XMLSchema#> .
<urn:test:resource> a sh:NodeShape ; sh:name "Customer description"@en ; sh:description "The supplied customer shape."@en ;
sh:targetClass <urn:Customer> ; sh:targetNode <urn:alice> ; sh:targetSubjectsOf <urn:customer> ; sh:targetObjectsOf <urn:customer> ;
sh:closed true ; sh:ignoredProperties (<urn:ignored>) ; sh:deactivated false ; sh:severity sh:Warning ;
sh:message "Provide the required customer details."@en ;
sh:property [ sh:path [ sh:alternativePath (<urn:first> [ sh:inversePath <urn:second> ]) ] ;
 sh:name "Customer name" ; sh:description "Name in this shape." ; sh:order 1 ; sh:group <urn:group> ; sh:defaultValue "Unknown" ;
 sh:datatype xsd:string ; sh:class <urn:Person> ; sh:nodeKind sh:IRI ; sh:minCount 1 ; sh:maxCount 1 ;
 sh:minExclusive 0 ; sh:maxExclusive 100 ; sh:minInclusive 1 ; sh:maxInclusive 99 ; sh:minLength 1 ; sh:maxLength 50 ;
 sh:pattern "^[A-Z]" ; sh:flags "i" ; sh:languageIn ("en" "nl") ; sh:uniqueLang true ;
 sh:equals <urn:equal> ; sh:disjoint <urn:other> ; sh:lessThan <urn:max> ; sh:lessThanOrEquals <urn:cap> ;
 sh:in ("First" "Second") ; sh:hasValue "First" ; sh:node <urn:NameShape> ; sh:not <urn:ExcludedShape> ;
 sh:or (<urn:One> <urn:Two>) ; sh:and (<urn:Three> <urn:Four>) ; sh:xone (<urn:Five> <urn:Six>) ;
 sh:qualifiedValueShape <urn:Qualified> ; sh:qualifiedMinCount 1 ; sh:qualifiedMaxCount 2 ; sh:qualifiedValueShapesDisjoint true ] ;
sh:sparql [ sh:prefixes <urn:prefixes> ; sh:select "SELECT $this WHERE { FILTER(false) }" ; sh:ask "ASK { FILTER(false) }" ; sh:message "Review the source SPARQL constraint." ] ;
sh:rule [ sh:condition <urn:Condition> ; sh:construct "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }" ] ;
sh:expression [ sh:filterShape <urn:Filter> ; sh:nodes sh:this ] ;
sh:zeroOrMorePath <urn:zero> ; sh:oneOrMorePath <urn:one> ; sh:zeroOrOnePath <urn:optional> .
"#], "en").await?;
    assert!(codes.is_empty());
    for predicate in [
        "targetClass",
        "targetNode",
        "targetSubjectsOf",
        "targetObjectsOf",
        "closed",
        "ignoredProperties",
        "deactivated",
        "severity",
        "message",
        "alternativePath",
        "inversePath",
        "datatype",
        "class",
        "nodeKind",
        "minCount",
        "maxCount",
        "minExclusive",
        "maxExclusive",
        "minInclusive",
        "maxInclusive",
        "minLength",
        "maxLength",
        "pattern",
        "flags",
        "languageIn",
        "uniqueLang",
        "equals",
        "disjoint",
        "lessThan",
        "lessThanOrEquals",
        "in",
        "hasValue",
        "node",
        "not",
        "or",
        "and",
        "xone",
        "qualifiedValueShape",
        "qualifiedMinCount",
        "qualifiedMaxCount",
        "qualifiedValueShapesDisjoint",
        "sparql",
        "select",
        "ask",
        "rule",
        "condition",
        "construct",
        "expression",
        "filterShape",
        "nodes",
        "zeroOrMorePath",
        "oneOrMorePath",
        "zeroOrOnePath",
    ] {
        assert!(
            body.contains(&format!("http://www.w3.org/ns/shacl#{predicate}")),
            "missing source predicate {predicate}"
        );
    }
    assert!(body.contains("SELECT $this WHERE"));
    assert!(body.contains("Customer name"));
    assert!(
        !body.contains("**Domain:**"),
        "SHACL targets must not become inferred property domains"
    );
    Ok(())
}

#[tokio::test]
async fn plain_annotation_line_wrapping_does_not_become_pdf_word_spacing()
-> Result<()> {
    let (wrapped, _) = document(
        &[
            "<urn:test:resource> rdfs:label \"Duty state\" ; rdfs:comment \
             \"\"\"Value is any skos:Concept. The state machine is\n    one \
             possible interpretation. Implementations are free to use a \
             different\n    state vocabulary.\"\"\"@en .",
        ],
        "en",
    )
    .await?;
    assert!(
        wrapped.contains(
            "The state machine is one possible interpretation. \
             Implementations are free to use a different state vocabulary."
        ),
        "source formatting whitespace must not become visible multi-space \
         gaps in prose"
    );
    Ok(())
}
