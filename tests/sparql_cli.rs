#[tokio::test]
async fn publication_commands_expose_endpoint_sources_and_transactional_stores()
-> anyhow::Result<()> {
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "collect", "--help"])
        .output()
        .await?;
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout)?;
    assert!(
        help.contains("sparql-endpoint"),
        "endpoint collection must be distinct from documenting a query file"
    );
    assert!(
        help.contains("--sparql-store"),
        "external KG configuration must be explicit"
    );
    Ok(())
}
