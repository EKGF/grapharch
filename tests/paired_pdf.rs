//! A specification page compiles a paired PDF of itself unless its target
//! opts out, for families whose named PDF editions already carry the same
//! document.
use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile, collect_all},
        dataset::Dataset,
        pdf_name::Edition,
        plan::TargetPlanner,
        render::Publisher,
        showcases,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
    std::{path::Path, sync::Arc},
};

const EDITION: &str = r##"
ex:project ga:target ex:edition .
ex:edition a ga:Target ; dct:title "Community edition" ;
    ga:scope ex:scope ; ga:profile ex:profile ;
    ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ; <https://ekgf.org/ontology/grapharch#documentType> <https://ekgf.org/ontology/grapharch#SpecificationDocument> ; <https://ekgf.org/ontology/grapharch#slug> "specification" ; <https://ekgf.org/ontology/grapharch#publisher> <urn:uuid:6c1408aa-039c-42f9-8dee-9c3f0009d61a> ;
    ga:destination "pdf" .
ex:target ga:metadataRow ex:edition-links .
ex:edition-links a ga:MetadataRow ; ga:order 1 ; ga:field "custom" ;
    dct:title "PDF editions:" ; ga:body """<a href="urn:specification:edition">Community edition</a>""" .
"##;

async fn publish(extra: &str, output: &Path) -> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    tokio::fs::write(
        &path,
        format!(
            "{}{EDITION}{extra}",
            include_str!("../examples/specification/project.ttl")
        ),
    )
    .await?;
    let project = "urn:specification:project";
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    collect_all(
        vec![Arc::new(RdfFile::new(
            project,
            "urn:specification:collector",
            path,
        )?) as Arc<dyn Collector>],
        &writer,
        1,
    )
    .await?;
    let revision = Assembler::new(dataset.clone()).assemble(project).await?;
    let planner = TargetPlanner::new(dataset.clone(), revision);
    let mut plans = Vec::new();
    for target in ["target", "edition"] {
        plans.push(
            planner
                .resolve(
                    project,
                    &NamedNode::new(format!("urn:specification:{target}"))?,
                    &writer,
                )
                .await?,
        );
    }
    drop(writer);
    worker.await??;
    source.close()?;
    showcases::publish(
        &Publisher::new(dataset).with_edition(Edition::Default),
        &plans,
        output,
    )
    .await?;
    Ok(())
}

fn pdfs(directory: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(directory)
        .expect("publication directory")
        .map(|entry| entry.expect("entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".pdf"))
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn specification_target_can_omit_its_paired_pdf() -> Result<()> {
    let output = tempfile::tempdir()?;
    publish(
        "ex:target <https://ekgf.org/ontology/grapharch#pairedPdf> false .\n",
        output.path(),
    )
    .await?;
    let specification = output.path().join("specification");
    assert!(
        pdfs(&specification).is_empty(),
        "no paired PDF is compiled beside the page"
    );
    let html =
        tokio::fs::read_to_string(specification.join("index.html")).await?;
    assert!(!html.contains("Specification PDF"));
    let document = scraper::Html::parse_document(&html);
    let downloads: Vec<_> = document
        .select(&scraper::Selector::parse("a.pdf-download").unwrap())
        .map(|link| link.value().attr("href").unwrap().to_owned())
        .collect();
    assert_eq!(
        downloads,
        ["../pdf/ekgf-spec-specification.pdf"],
        "the named edition remains the one PDF download"
    );
    let manifest =
        tokio::fs::read_to_string(specification.join("manifest.yaml")).await?;
    assert!(!manifest.contains(".pdf"), "{manifest}");
    assert_eq!(pdfs(&output.path().join("pdf")), [
        "ekgf-spec-specification.pdf"
    ]);
    Ok(())
}

#[tokio::test]
async fn specification_target_keeps_its_paired_pdf_by_default() -> Result<()> {
    let output = tempfile::tempdir()?;
    publish("", output.path()).await?;
    assert_eq!(
        pdfs(&output.path().join("specification")).len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn paired_pdf_is_rejected_outside_specification_targets() -> Result<()> {
    let output = tempfile::tempdir()?;
    let error = publish(
        "ex:edition <https://ekgf.org/ontology/grapharch#pairedPdf> false .\n",
        output.path(),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("pairedPdf"),
        "{error:#}"
    );
    Ok(())
}
