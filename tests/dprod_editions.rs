#[path = "support/definitions.rs"]
mod definitions;

use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile, collect_all},
        dataset::Dataset,
        plan::TargetPlanner,
        render::Publisher,
        showcases,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
    std::{path::Path, sync::Arc},
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
    let output = tempfile::tempdir()?;
    render_dual_editions(output.path(), false).await
}

#[tokio::test]
#[ignore = "retained PDF visual acceptance artifacts"]
async fn review_dual_edition_pdfs() -> Result<()> {
    render_dual_editions(Path::new("output/pdf/dprod-editions"), false).await
}

#[tokio::test]
async fn omg_cover_metadata_can_continue_without_a_numbered_footer()
-> Result<()> {
    let output = tempfile::tempdir()?;
    render_dual_editions(output.path(), true).await
}

async fn render_dual_editions(
    output: &Path,
    long_metadata: bool,
) -> Result<()> {
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
    ga:scope ex:scope ; ga:profile ex:submission-profile ; ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ;
    ga:destination "pdf/omg" ; ga:publisher ex:omg-authority ;
    ga:publicationStatus "Submission Preview" ;
    ga:publicationDate "2026-10-24"^^<http://www.w3.org/2001/XMLSchema#date> ;
    ga:thisVersion <https://example.org/dprod/2026-10-24/> ;
    ga:latestVersion <https://example.org/dprod/> .
ex:submission-profile a ga:Profile ; ga:style "omg" ;
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
    if long_metadata {
        ttl = ttl.replace(
            "DPROD OMG submission preview",
            &format!(
                "DPROD OMG submission preview {}",
                "A long explicitly supplied publication title. ".repeat(80)
            ),
        );
    }
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
    let publications =
        showcases::publish(&Publisher::new(restored), &plans, output).await?;
    assert_eq!(publications.len(), 4);
    let revisions: std::collections::HashSet<_> = publications
        .iter()
        .map(|p| p.manifest.input_revision.as_str())
        .collect();
    assert_eq!(revisions.len(), 1);
    let community =
        tokio::fs::read_to_string(output.join("pdf/ekgf/main.typ")).await?;
    let submission =
        tokio::fs::read_to_string(output.join("pdf/omg/main.typ")).await?;
    for body in [&community, &submission] {
        assert!(body.contains("managed, and governed collection of data"));
        assert!(body.contains("Document provenance"));
    }
    assert!(community.contains("Community Draft"));
    assert!(!community.contains("DPROD OMG submission preview"));
    assert!(submission.contains("DPROD OMG submission preview"));
    assert!(submission.contains("Submission Preview"));
    assert!(
        submission.contains("#set text(font: \"Liberation Serif\", size: 10pt"),
        "OMG body follows the Times-compatible 10pt reference typography"
    );
    assert!(
        submission
            .contains("#show heading.where(level: 4): set block(below: 10pt)"),
        "deep headings need separation from following content"
    );
    assert!(
        submission.contains("#show heading: set block(inset: (bottom: 10pt))"),
        "heading separation must survive the fragment container boundary"
    );
    let pdf = tokio::fs::read(output.join("pdf/omg/document.pdf")).await?;
    assert!(
        String::from_utf8_lossy(&pdf).contains("LiberationSerif"),
        "the portable Times-compatible font must be embedded"
    );

    assert!(submission.contains("#let ga-style = \"omg\""));
    assert!(community.contains("#let ga-style = \"reader\""));
    assert!(submission.contains("#set page(numbering: \"i\")"));
    assert!(submission.contains("#set page(numbering: \"1\")"));
    assert!(submission.contains("not an approved OMG specification"));
    for relative in ["pdf/ekgf/document.pdf", "pdf/omg/document.pdf"] {
        assert!(
            tokio::fs::read(output.join(relative))
                .await?
                .starts_with(b"%PDF-")
        );
    }
    Ok(())
}

/// Authored links between sibling targets stay relative to the published page.
/// The same output then works in production, the showcase gallery, previews
/// and local builds; `ga:thisVersion` is canonical identity, never a link base.
#[tokio::test]
async fn edition_links_resolve_to_sibling_pdfs_in_the_build_output()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let fixture = format!(
        r##"{}
ex:project ga:target ex:ekgf-edition, ex:omg-edition .
ex:ekgf-edition a ga:Target ; dct:title "Community edition" ;
    ga:scope ex:scope ; ga:profile ex:profile ;
    ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ;
    ga:destination "pdf/ekgf" .
ex:omg-edition a ga:Target ; dct:title "Submission-preview edition" ;
    ga:scope ex:scope ; ga:profile ex:profile ;
    ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ;
    ga:destination "pdf/omg" .
ex:target ga:metadataRow ex:version-link, ex:latest-link, ex:edition-links .
ex:version-link a ga:MetadataRow ; ga:order 1 ; ga:field "thisVersion" ; dct:title "This version:" .
ex:latest-link a ga:MetadataRow ; ga:order 2 ; ga:field "latestVersion" ; dct:title "Latest version:" .
ex:edition-links a ga:MetadataRow ; ga:order 3 ; ga:field "custom" ;
    dct:title "PDF editions:" ; ga:body """<a href="../pdf/ekgf/document.pdf">EKGF community PDF</a><br><a href="../pdf/omg/document.pdf">OMG submission-preview PDF</a>""" .
"##,
        include_str!("../examples/specification/project.ttl")
    );
    tokio::fs::write(&path, fixture).await?;
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
    for target in ["target", "ekgf-edition", "omg-edition"] {
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
    let output = tempfile::tempdir()?;
    showcases::publish(&Publisher::new(dataset), &plans, output.path()).await?;
    let page = output.path().join("specification/index.html");
    let html = tokio::fs::read_to_string(&page).await?;
    let document = scraper::Html::parse_document(&html);
    let download = document
        .select(&scraper::Selector::parse("a.pdf-download").unwrap())
        .next()
        .unwrap();
    assert!(
        download
            .text()
            .collect::<String>()
            .contains("Specification PDF"),
        "the attached reference PDF must be distinguished from named editions"
    );
    assert!(
        download
            .attr("aria-label")
            .unwrap()
            .to_lowercase()
            .contains("specification")
    );
    let selector = scraper::Selector::parse(
        "a.pdf-download, dl.publication-metadata a[href$=\".pdf\"]",
    )
    .unwrap();
    let hrefs: Vec<_> = document
        .select(&selector)
        .map(|link| link.attr("href").unwrap().to_owned())
        .collect();
    assert_eq!(
        hrefs.len(),
        5,
        "both named editions must have visible PDF download controls \
         alongside the attached reference, besides their metadata links: \
         {hrefs:?}"
    );
    let base = url::Url::from_file_path(&page).unwrap();
    for href in &hrefs {
        let resolved = base.join(href)?;
        assert_eq!(
            resolved.scheme(),
            "file",
            "{href} must resolve relative to the published page, not to the \
             canonical publication URL"
        );
        let file = resolved.to_file_path().unwrap();
        assert!(
            file.starts_with(output.path()) &&
                tokio::fs::read(&file).await?.starts_with(b"%PDF-"),
            "{href} must name a PDF in the build output: {}",
            file.display()
        );
    }
    let typst =
        tokio::fs::read_to_string(output.path().join("specification/main.typ"))
            .await?;
    for edition in ["ekgf", "omg"] {
        assert!(
            typst.contains(&format!(
                "#link(\"../pdf/{edition}/document.pdf\")"
            )),
            "the specification PDF beside index.html carries the same \
             relative edition link"
        );
    }
    Ok(())
}
