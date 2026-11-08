use {
    anyhow::Result,
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
    std::path::Path,
};

const COMMIT: &str = "33a92cd34285d532c158da97bb42add089f135d3";

#[tokio::test]
async fn fibo_be_inventory_is_complete_pinned_and_does_not_collect_other_domains()
-> Result<()> {
    let config: serde_yaml::Value = serde_yaml::from_str(
        &tokio::fs::read_to_string("examples/fibo-be/showcase.yaml").await?,
    )?;
    assert_eq!(
        config["source"]["kind"].as_str(),
        Some("rdf-urls")
    );
    let documents = config["source"]["documents"].as_sequence().unwrap();
    assert_eq!(documents.len(), 48);
    let mut urls = std::collections::BTreeSet::new(); // bounded: the 48 explicitly registered source documents.
    for document in documents {
        let url = document["url"].as_str().unwrap();
        assert!(url.starts_with(&format!("https://raw.githubusercontent.com/edmcouncil/fibo/{COMMIT}/BE/")) && url.ends_with(".rdf"));
        assert!(urls.insert(url));
        assert_eq!(
            document["media_type"].as_str(),
            Some("application/rdf+xml")
        );
    }
    for path in [
        "AllBE.rdf",
        "AllBE-Europe.rdf",
        "AllBE-NorthAmerica.rdf",
        "AllBE-ReferenceIndividuals.rdf",
        "Corporations/Corporations.rdf",
        "LegalEntities/LegalPersons.rdf",
        "GovernmentEntities/AsianJurisdiction/\
         EasternAsiaGovernmentEntitiesAndJurisdictions.rdf",
        "Trusts/Trusts.rdf",
    ] {
        assert!(
            urls.iter().any(|url| url.ends_with(path)),
            "missing {path}"
        );
    }
    assert!(!tokio::fs::try_exists("examples/fibo-be/sources").await?);
    let definitions =
        tokio::fs::read_to_string("examples/fibo-be/project.ttl").await?;
    assert_eq!(
        definitions.matches("gov:inFamily ex:family").count(),
        48
    );
    assert!(definitions.contains(COMMIT));
    let registry = tokio::fs::read_to_string("examples/showcases.ttl").await?;
    assert!(registry.contains("ga:source \"fibo-be/showcase.yaml\""));
    Ok(())
}

#[tokio::test]
#[ignore = "live pinned FIBO BE acceptance; requires network and PlantUML"]
async fn live_fibo_be_renders_html_and_pdf_from_restored_graph() -> Result<()> {
    let started = std::time::Instant::now();
    let output = Path::new("output/fibo-be-review");
    tokio::fs::create_dir_all(output).await?;
    let config = tempfile::tempdir()?;
    for file in ["project.ttl", "showcase.yaml"] {
        tokio::fs::copy(
            Path::new("examples/fibo-be").join(file),
            config.path().join(file),
        )
        .await?;
    }
    let (dataset, plans) = showcases::family(
        &config.path().join("showcase.yaml"),
        Path::new("output/cache/git"),
    )
    .await?;
    assert_eq!(plans.len(), 2);
    let archive = output.join("documentation.nq");
    dataset.export(archive.clone()).await?;
    config.close()?;
    drop(dataset);
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output).await?;
    assert_eq!(publications.len(), 2);
    let html =
        tokio::fs::read_to_string(output.join("specification/index.html"))
            .await?;
    assert!(html.contains("48 ontologies"));
    assert!(html.contains("Business Entities"));
    assert!(
        tokio::fs::read(output.join("pdf/document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    let typst = tokio::fs::read_to_string(output.join("pdf/main.typ")).await?;
    assert!(typst.contains("LegalPerson") && typst.contains("Corporation"));
    assert!(
        typst.contains("Permission is hereby granted"),
        "original MIT notice must remain"
    );
    eprintln!(
        "FIBO BE collection, restore and HTML/PDF: {:.2}s",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
