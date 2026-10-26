use anyhow::Result;

#[tokio::test]
async fn verification_cli_checks_every_case_and_keeps_failure_evidence()
-> Result<()> {
    let root = tempfile::tempdir()?;
    std::fs::write(
        root.path().join("showcases.ttl"),
        r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
<urn:gallery:project> ga:showcase <urn:case:first>, <urn:case:second> .
<urn:case:first> a ga:Showcase; ga:slug "first"; ga:kind "authored"; ga:source "missing-first.ttl" .
<urn:case:second> a ga:Showcase; ga:slug "second"; ga:kind "authored"; ga:source "missing-second.ttl" .
"#,
    )?;
    let output = tempfile::tempdir()?;
    let result = tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .args(["publication", "verify-gallery", "--source"])
        .arg(root.path())
        .args(["--project", "urn:gallery:project", "--output-root"])
        .arg(output.path())
        .arg("--json")
        .output()
        .await?;
    assert!(
        !result.status.success(),
        "unexpected showcase failures must fail CI"
    );
    let report_path = output.path().join("report.json");
    assert!(
        report_path.is_file(),
        "failed verification must retain its report: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report_path)?)?;
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["succeeded"], false);
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(
        cases.len(),
        2,
        "the first failure must not abandon remaining cases"
    );
    for id in ["urn:case:first", "urn:case:second"] {
        assert!(
            cases
                .iter()
                .any(|case| case["id"] == id && case["succeeded"] == false)
        );
    }
    let evidence =
        std::fs::read_to_string(output.path().join("verification.nq"))?;
    assert!(
        evidence.contains("urn:case:first") &&
            evidence.contains("urn:case:second")
    );
    assert!(evidence.contains("Diagnostic"));
    let stdout: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(stdout, report);
    Ok(())
}
