use {
    anyhow::Result,
    grapharch::publication::ontology_family,
    std::path::Path,
};

fn copy(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn long_rdf_collections_are_sequences_not_deep_expressions() -> Result<()>
{
    let source = tempfile::tempdir()?;
    copy(
        Path::new("examples/ekg-ontologies"),
        source.path(),
    )?;
    let path = source.path().join("sources/ekgf-shape.ttl");
    let mut content = std::fs::read_to_string(&path)?;
    let choices = (0..48)
        .map(|i| format!("\"choice{i:03}\""))
        .collect::<Vec<_>>()
        .join(" ");
    content.push_str(&format!(r#"
<https://ekgf.org/ontology/shape#LongEnumeration>
    a <http://www.w3.org/ns/shacl#NodeShape> ;
    <http://www.w3.org/2000/01/rdf-schema#label> "Long enumeration"@en ;
    <http://www.w3.org/2000/01/rdf-schema#comment> "A shape with a long, shallow RDF collection."@en ;
    <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> <https://ekgf.org/ontology/shape#> ;
    <http://www.w3.org/ns/shacl#in> ({choices}) .
"#));
    std::fs::write(path, content)?;
    let built = ontology_family::collect(
        &source.path().join("sources"),
        &source.path().join("project.ttl"),
        "urn:ekgf-family:project",
        "urn:ekgf-family:specification",
        4,
    )
    .await?;
    let mut rows = built.dataset.select("PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?body WHERE { GRAPH ?g { ?fragment ga:subject <https://ekgf.org/ontology/shape#LongEnumeration> ; ga:body ?body } }".into()).await?;
    let row = rows.next().await?.expect("documented long enumeration");
    let oxrdf::Term::Literal(body) = &row["body"] else {
        panic!("fragment body must be a literal")
    };
    let mut previous = 0;
    for i in 0..48 {
        let offset = body
            .value()
            .find(&format!("choice{i:03}"))
            .expect("every list item is documented");
        assert!(
            i == 0 || offset > previous,
            "RDF list order must be preserved"
        );
        previous = offset;
    }
    assert!(
        !body.value().contains("rdf:rest"),
        "list plumbing must not replace the readable sequence"
    );
    Ok(())
}

async fn expression_error(expression: &str) -> Result<String> {
    let source = tempfile::tempdir()?;
    copy(
        Path::new("examples/ekg-ontologies"),
        source.path(),
    )?;
    let path = source.path().join("sources/ekgf-shape.ttl");
    let mut content = std::fs::read_to_string(&path)?;
    content.push_str(&format!(r#"
<https://ekgf.org/ontology/shape#CollectionGuard>
    a <http://www.w3.org/ns/shacl#NodeShape> ;
    <http://www.w3.org/2000/01/rdf-schema#label> "Collection guard"@en ;
    <http://www.w3.org/2000/01/rdf-schema#comment> "An explicitly malformed expression for a regression."@en ;
    <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> <https://ekgf.org/ontology/shape#> ;
    {expression}
"#));
    std::fs::write(path, content)?;
    let result = ontology_family::collect(
        &source.path().join("sources"),
        &source.path().join("project.ttl"),
        "urn:ekgf-family:project",
        "urn:ekgf-family:specification",
        4,
    )
    .await;
    match result {
        Ok(_) => anyhow::bail!("malformed expression must fail"),
        Err(error) => Ok(format!("{error:#}")),
    }
}

#[tokio::test]
async fn cyclic_rdf_collections_fail_with_the_documented_subject() -> Result<()>
{
    let error = expression_error(
        r#"
        <http://www.w3.org/ns/shacl#in> _:cycle .
        _:cycle <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> "choice" ;
                <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest> _:cycle .
    "#,
    )
    .await?;
    assert!(error.contains("cyclic RDF collection"), "{error}");
    assert!(error.contains("shape#CollectionGuard"), "{error}");
    Ok(())
}

#[tokio::test]
async fn flattening_lists_does_not_remove_expression_depth_protection()
-> Result<()> {
    let mut expression = String::from("<urn:example:next> ");
    for _ in 0..26 {
        expression.push_str("[ <urn:example:next> ");
    }
    expression.push_str("\"leaf\"");
    for _ in 0..26 {
        expression.push_str(" ]");
    }
    expression.push_str(" .");
    let error = expression_error(&expression).await?;
    assert!(
        error.contains("anonymous RDF expression exceeds depth budget"),
        "{error}"
    );
    assert!(error.contains("shape#CollectionGuard"), "{error}");
    Ok(())
}
