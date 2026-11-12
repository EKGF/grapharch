//! Tagged structure, conformance and document metadata of generated PDFs.
#[path = "support/pdf.rs"]
mod pdf;

use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        showcases,
    },
    pdf::PdfFacts,
    std::path::{Path, PathBuf},
};

async fn publish(
    publisher: &Publisher,
    plan: &grapharch::publication::plan::ResolvedPlan,
    out: &Path,
) -> Result<PathBuf> {
    let PublishOutcome::Published(publication) =
        publisher.render(plan, out).await?
    else {
        anyhow::bail!("publication required")
    };
    Ok(publication.directory)
}

#[tokio::test]
async fn pdf_without_a_publication_date_embeds_no_creation_date() -> Result<()>
{
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    tokio::fs::write(
        &path,
        include_str!("../examples/showcase/project.ttl"),
    )
    .await?;
    let (dataset, plans) = showcases::authored(&path).await?;
    source.close()?;
    let out = tempfile::tempdir()?;
    let directory =
        publish(&Publisher::new(dataset), &plans[2], out.path()).await?;
    let facts = PdfFacts::of(&directory.join("document.pdf"))?;
    assert!(facts.marked && facts.structure_tree);
    assert_eq!(
        facts.title.as_deref(),
        Some("Documentation graph - review draft")
    );
    assert_eq!(
        facts.created, None,
        "no wall-clock date may be invented for an undated publication"
    );
    assert_eq!(facts.keywords, None);
    Ok(())
}

#[tokio::test]
async fn wrapped_table_identifiers_keep_their_extracted_text_intact()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    let identifier = "dprod-shapes:RejectMixedLifecycleStatusOnDataOfferShape";
    let fixture = include_str!("../examples/showcase/project.ttl").replace(
        "Literal brackets such as",
        // The Turtle literal keeps Markdown line breaks as `\n` escapes.
        &format!(
            "| Shape | Code |\\n| --- | --- |\\n| {identifier} | \
             `{identifier}` |\\n\\nLiteral brackets such as"
        ),
    );
    tokio::fs::write(&path, fixture).await?;
    let (dataset, plans) = showcases::authored(&path).await?;
    source.close()?;
    let out = tempfile::tempdir()?;
    let directory =
        publish(&Publisher::new(dataset), &plans[2], out.path()).await?;
    let typst = tokio::fs::read_to_string(directory.join("main.typ")).await?;
    assert!(
        typst.contains("#ga-wrap(") && typst.contains("#ga-wrap-raw("),
        "long table identifiers need line-break opportunities"
    );
    assert!(
        !typst.contains('\u{200b}'),
        "break opportunities must not be characters of the identifier"
    );
    let facts = PdfFacts::of(&directory.join("document.pdf"))?;
    assert!(
        !facts.zero_width_text,
        "copied, searched and spoken identifiers must not contain U+200B"
    );
    Ok(())
}
