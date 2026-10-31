#[path = "support/definitions.rs"]
mod definitions;

use {
    anyhow::Result,
    grapharch::publication::{dataset::Dataset, render::Publisher, showcases},
    std::path::Path,
};

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_tree(
                &entry.path(),
                &destination.join(entry.file_name()),
            )?;
        } else {
            std::fs::copy(entry.path(), destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn declared_edition_targets_render_from_one_restored_source_revision()
-> Result<()> {
    let source = tempfile::tempdir()?;
    copy_tree(Path::new("examples/dprod"), source.path())?;
    let definitions = source.path().join("project.ttl");
    let mut ttl = definitions::set_property(
        &std::fs::read_to_string(&definitions)?,
        "urn:dprod-showcase:pdf",
        "https://ekgf.org/ontology/grapharch#destination",
        definitions::literal("pdf/ekgf"),
    )?;
    ttl.push_str(
        r##"
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
ex:project ga:target ex:submission .
ex:pdf ga:publicationStatus "Community Draft" .
ex:submission a ga:Target ; dct:title "DPROD OMG submission preview" ;
    ga:subtitle "Prepared for review; not an approved OMG specification" ;
    ga:scope ex:scope ; ga:profile ex:submission-profile ; ga:renderer "typst" ;
    ga:destination "pdf/omg" ; ga:publisher ex:omg-authority ;
    ga:publicationStatus "Submission Preview" .
ex:submission-profile a ga:Profile ; ga:style "reader" ;
    ga:placement <urn:dprod-showcase:pdf-placement:abstract>,
        <urn:dprod-showcase:pdf-placement:overview>,
        <urn:dprod-showcase:pdf-placement:classes>,
        <urn:dprod-showcase:pdf-placement:example> .
ex:omg-authority a <https://ekgf.org/ontology/artifact-governance#Authority> ;
    rdfs:label "Object Management Group submission preview" ;
    <https://ekgf.org/ontology/artifact-governance#authorityAcronym> "OMG" ;
    ga:accentColor "#004c6d" ; ga:url "https://www.omg.org/" .
"##,
    );
    std::fs::write(definitions, ttl)?;
    let (dataset, plans) = showcases::dprod(source.path()).await?;
    assert_eq!(
        plans.len(),
        4,
        "every declared active target must be resolved"
    );
    let saved = tempfile::tempdir()?;
    let archive = saved.path().join("source.nq");
    dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let output = tempfile::tempdir()?;
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output.path())
            .await?;
    assert_eq!(publications.len(), 4);
    let revisions: std::collections::HashSet<_> = publications
        .iter()
        .map(|p| p.manifest.input_revision.as_str())
        .collect();
    assert_eq!(revisions.len(), 1);
    let community =
        tokio::fs::read_to_string(output.path().join("pdf/ekgf/main.typ"))
            .await?;
    let submission =
        tokio::fs::read_to_string(output.path().join("pdf/omg/main.typ"))
            .await?;
    for body in [&community, &submission] {
        assert!(body.contains("managed, and governed collection of data"));
        assert!(body.contains("Document provenance"));
    }
    assert!(community.contains("Community Draft"));
    assert!(!community.contains("DPROD OMG submission preview"));
    assert!(submission.contains("DPROD OMG submission preview"));
    assert!(submission.contains("Submission Preview"));
    assert!(submission.contains("not an approved OMG specification"));
    for relative in ["pdf/ekgf/document.pdf", "pdf/omg/document.pdf"] {
        assert!(
            tokio::fs::read(output.path().join(relative))
                .await?
                .starts_with(b"%PDF-")
        );
    }
    Ok(())
}

#[tokio::test]
async fn edition_download_links_resolve_against_the_publication_url()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = format!(
        r##"{}
ex:target ga:metadataRow ex:version-link, ex:latest-link, ex:edition-links .
ex:version-link a ga:MetadataRow ; ga:order 1 ; ga:field "thisVersion" ; dct:title "This version:" .
ex:latest-link a ga:MetadataRow ; ga:order 2 ; ga:field "latestVersion" ; dct:title "Latest version:" .
ex:edition-links a ga:MetadataRow ; ga:order 3 ; ga:field "custom" ;
    dct:title "PDF editions:" ; ga:body """<a href="../pdf/ekgf/document.pdf">EKGF community PDF</a><br><a href="../pdf/omg/document.pdf">OMG submission-preview PDF</a>""" .
"##,
        include_str!("../examples/specification/project.ttl")
    );
    tokio::fs::write(&path, fixture).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    source.close()?;
    let output = tempfile::tempdir()?;
    let publications =
        showcases::publish(&Publisher::new(dataset), &plans, output.path())
            .await?;
    let html =
        tokio::fs::read_to_string(publications[0].directory.join("index.html"))
            .await?;
    for edition in ["ekgf", "omg"] {
        assert!(html.contains(&format!(
            "href=\"https://example.org/spec/pdf/{edition}/document.pdf\""
        )));
    }
    Ok(())
}
