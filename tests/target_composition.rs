//! A target may contain other targets: their destinations nest inside its own,
//! republishing it preserves them, and links to a target IRI resolve to the
//! relative path between the destinations.
use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        collect::{Collector, RdfFile},
        dataset::Dataset,
        plan::{ResolvedPlan, TargetPlanner},
        render::{PublishOutcome, Publisher},
        writer::GraphWriter,
    },
    std::path::Path,
};

const BASE: &str = include_str!("../examples/specification/project.ttl");

/// The specification example nested inside a containing `site` target, with a
/// sibling PDF edition, links to both by IRI, and an optional declaration.
fn definitions(contains: &str, extra: &str) -> String {
    let base = BASE
        .replace(
            "ga:scope ex:scope ; ga:target ex:target .",
            "ga:scope ex:scope ; ga:target ex:target, ex:site, ex:pdf .",
        )
        .replace(
            "ga:destination \"specification\" ;",
            "ga:destination \"site/spec/main\" ;",
        )
        .replace(
            "The string `<example>` is rendered as literal source text.",
            "The string `<example>` is rendered as literal source text. Read \
             the [PDF edition](urn:specification:pdf).",
        );
    format!(
        r#"{base}
ex:site a ga:Target ; dct:title "Containing site" ; ga:scope ex:scope ;
    ga:profile ex:profile ; ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer> ;
    ga:documentType ga:SpecificationDocument ; ga:slug "site" ;
    ga:destination "site" ; ga:publisher <urn:uuid:6c1408aa-039c-42f9-8dee-9c3f0009d61a> ;
    ga:publicationStatus "Editor's Draft" ; ga:publicationDate "2026-10-24"^^xsd:date ;
    ga:thisVersion <https://example.org/> ; ga:latestVersion <https://example.org/> ;
    ga:license <https://creativecommons.org/licenses/by/4.0/> {contains} .
ex:pdf a ga:Target ; dct:title "PDF edition" ; ga:scope ex:scope ; ga:profile ex:profile ;
    ga:renderer <https://ekgf.org/ontology/grapharch#TypstRenderer> ; ga:destination "site/pdf" ;
    ga:documentType ga:SpecificationDocument ; ga:slug "family" ;
    ga:publisher <urn:uuid:6c1408aa-039c-42f9-8dee-9c3f0009d61a> .
ex:target ga:metadataRow ex:version-row, ex:latest-row, ex:editions-row .
ex:version-row a ga:MetadataRow ; ga:order 1 ; ga:field "thisVersion" ; dct:title "This version:" .
ex:latest-row a ga:MetadataRow ; ga:order 2 ; ga:field "latestVersion" ; dct:title "Latest version:" .
ex:editions-row a ga:MetadataRow ; ga:order 3 ; ga:field "custom" ; dct:title "Editions:" ;
    ga:body """<a href="urn:specification:site">Home</a> · <a href="urn:specification:pdf">PDF</a>""" .
{extra}
"#
    )
}

const PROJECT: &str = "urn:specification:project";

async fn plans(
    root: &Path,
    text: &str,
) -> Result<(Dataset, Vec<ResolvedPlan>)> {
    let path = root.join("project.ttl");
    std::fs::write(&path, text)?;
    let dataset = Dataset::memory()?;
    let (writer, worker) =
        GraphWriter::spawn(dataset.clone(), 8, 4 * 1024 * 1024)?;
    let result = async {
        RdfFile::new(PROJECT, "urn:specification:collector", path)?
            .collect(&writer)
            .await?;
        let revision =
            Assembler::new(dataset.clone()).assemble(PROJECT).await?;
        let planner = TargetPlanner::new(dataset.clone(), revision);
        let mut plans = Vec::new();
        for target in ["target", "site", "pdf"] {
            plans.push(
                planner
                    .resolve(
                        PROJECT,
                        &oxrdf::NamedNode::new(format!(
                            "urn:specification:{target}"
                        ))?,
                        &writer,
                    )
                    .await?,
            );
        }
        anyhow::Ok(plans)
    }
    .await;
    drop(writer);
    worker.await??;
    Ok((dataset, result?))
}

async fn publish(
    publisher: &Publisher,
    plan: &ResolvedPlan,
    output: &Path,
) -> Result<()> {
    let PublishOutcome::Published(_) = publisher.render(plan, output).await?
    else {
        panic!("expected a publication")
    };
    Ok(())
}

#[tokio::test]
async fn nested_destinations_require_a_declared_containing_target() -> Result<()>
{
    let root = tempfile::tempdir()?;
    let failure = plans(root.path(), &definitions("", ""))
        .await
        .err()
        .expect("undeclared nesting overlaps");
    assert!(
        format!("{failure:#}").contains("overlapping output destinations"),
        "{failure:#}"
    );
    let failure = plans(
        root.path(),
        &definitions(
            "; ga:containsTarget ex:target, ex:pdf",
            r#"ex:pdf ga:destination "site" ."#,
        ),
    )
    .await
    .err()
    .expect("a contained target cannot share the containing destination");
    assert!(
        format!("{failure:#}").contains("destination"),
        "{failure:#}"
    );
    Ok(())
}

#[tokio::test]
async fn republishing_a_containing_target_preserves_its_nested_targets()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let (dataset, plans) = plans(
        root.path(),
        &definitions("; ga:containsTarget ex:target, ex:pdf", ""),
    )
    .await?;
    let output = root.path().join("output");
    std::fs::create_dir(&output)?;
    let publisher = Publisher::new(dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
    let (spec, site, pdf) = (&plans[0], &plans[1], &plans[2]);
    // Nested targets first, then the container, then everything again. Every
    // target published so far keeps its files.
    let mut published = std::collections::BTreeSet::new();
    for (plan, file) in [
        (spec, "site/spec/main/index.html"),
        (pdf, "site/pdf/ekgf-spec-family.pdf"),
        (site, "site/index.html"),
        (spec, "site/spec/main/index.html"),
        (site, "site/index.html"),
        (pdf, "site/pdf/ekgf-spec-family.pdf"),
        (site, "site/index.html"),
    ] {
        publish(&publisher, plan, &output).await?;
        published.insert(file);
        for file in &published {
            assert!(output.join(file).is_file(), "{file} was lost");
        }
    }
    let spec =
        std::fs::read_to_string(output.join("site/spec/main/index.html"))?;
    assert!(
        spec.contains(r#"href="../../index.html""#),
        "{spec}"
    );
    assert!(
        spec.contains(r#"href="../../pdf/ekgf-spec-family.pdf""#),
        "{spec}"
    );
    let site = std::fs::read_to_string(output.join("site/index.html"))?;
    assert!(
        site.contains(r#"href="pdf/ekgf-spec-family.pdf""#),
        "{site}"
    );
    Ok(())
}

/// A link by target IRI to a PDF target names the PDF of this build's
/// edition, so a preview links its own PDF rather than the default branch's.
#[tokio::test]
async fn links_to_a_pdf_target_follow_the_build_edition() -> Result<()> {
    let root = tempfile::tempdir()?;
    let (dataset, plans) = plans(
        root.path(),
        &definitions("; ga:containsTarget ex:target, ex:pdf", ""),
    )
    .await?;
    let output = root.path().join("output");
    std::fs::create_dir(&output)?;
    let publisher = Publisher::new(dataset).with_edition(
        grapharch::publication::pdf_name::Edition::parse("pr-7")?,
    );
    for plan in &plans {
        publish(&publisher, plan, &output).await?;
    }
    assert!(output.join("site/pdf/ekgf-spec-family-pr-7.pdf").is_file());
    let site = std::fs::read_to_string(output.join("site/index.html"))?;
    assert!(
        site.contains(r#"href="pdf/ekgf-spec-family-pr-7.pdf""#),
        "{site}"
    );
    Ok(())
}

#[tokio::test]
async fn links_to_unregistered_targets_fail() -> Result<()> {
    let root = tempfile::tempdir()?;
    let text = definitions("; ga:containsTarget ex:target, ex:pdf", "")
        .replace(
            r#"<a href="urn:specification:pdf">PDF</a>"#,
            r#"<a href="urn:specification:retired">PDF</a>"#,
        );
    let text = format!(
        "{text}\nex:retired a ga:Target ; ga:scope ex:scope ; ga:profile ex:profile ; ga:renderer <https://ekgf.org/ontology/grapharch#SpecificationRenderer> ; ga:destination \"retired\" .\n"
    );
    let (dataset, plans) = plans(root.path(), &text).await?;
    let failure = Publisher::new(dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&plans[0], &root.path().join("output"))
        .await
        .err()
        .expect("an unregistered target link fails");
    assert!(
        format!("{failure:#}").contains("urn:specification:retired"),
        "{failure:#}"
    );
    Ok(())
}
