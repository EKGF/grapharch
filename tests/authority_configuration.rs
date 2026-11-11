use anyhow::Result;

#[tokio::test]
async fn shipped_publication_definitions_use_the_imported_authority_class()
-> Result<()> {
    // bounded: the authored publication definitions, not copied ontology
    // corpora.
    for path in [
        "docs/grapharch.ttl",
        "examples/specification/project.ttl",
        "examples/dprod/definitions.ttl",
        "examples/skos/project.ttl",
        "examples/fibo/prepare.py",
    ] {
        let source = tokio::fs::read_to_string(path).await?;
        assert!(
            !source.contains("ga:Authority"),
            "{path} must not use the removed alias"
        );
        assert!(
            !source.contains("https://ekgf.org/ontology/grapharch#Authority"),
            "{path} must not use the removed alias IRI"
        );
        assert!(
            source.contains(
                "https://ekgf.org/ontology/artifact-governance#Authority"
            ) || source.contains("artgov:Authority"),
            "{path} must name the imported authority class"
        );
    }
    Ok(())
}
