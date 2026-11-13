//! Tagged structure, conformance and document metadata of generated PDFs.
use grapharch::publication::render::typst_source;
#[path = "support/pdf.rs"]
mod pdf;

use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        ontology_family,
        render::{PublishOutcome, Publisher},
        showcases,
    },
    pdf::PdfFacts,
    std::path::{Path, PathBuf},
};

const SPECIFICATION_METADATA: &str =
    "\nex:target ga:keyword \"publication\", \"documentation\" .\n";

async fn specification(
    extra: &str,
) -> Result<(
    Dataset,
    Vec<grapharch::publication::plan::ResolvedPlan>,
)> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    tokio::fs::write(
        &path,
        format!(
            "{}{extra}",
            include_str!("../examples/specification/project.ttl")
        ),
    )
    .await?;
    let built = showcases::specification(&path).await?;
    source.close()?;
    Ok(built)
}

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
async fn specification_pdf_is_tagged_with_declared_document_metadata()
-> Result<()> {
    let (dataset, plans) = specification(SPECIFICATION_METADATA).await?;
    let publisher = Publisher::new(dataset)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
    let first = tempfile::tempdir()?;
    let directory = publish(&publisher, &plans[0], first.path()).await?;
    let bytes = tokio::fs::read(only_pdf(&directory)).await?;
    let facts = PdfFacts::read(bytes.clone())?;
    assert!(
        facts.marked,
        "the PDF must declare MarkInfo Marked"
    );
    assert!(
        facts.structure_tree,
        "the PDF must carry a logical structure tree"
    );
    assert_eq!(facts.language.as_deref(), Some("en"));
    assert_eq!(
        facts.title.as_deref(),
        Some("Documentation Publication Model 1.0")
    );
    assert_eq!(
        facts.author.as_deref(),
        Some("Enterprise Knowledge Graph Forum"),
        "without declared authors the publishing authority is the author"
    );
    assert_eq!(
        facts.keywords.as_deref(),
        Some("documentation, publication"),
        "declared keywords are embedded in a deterministic order"
    );
    assert_eq!(
        facts.created,
        Some((2026, 10, 24)),
        "the creation date is the declared publication date"
    );
    assert!(
        facts.xmp.contains(
            "<pdf:Keywords>documentation, publication</pdf:Keywords>"
        ),
        "XMP must repeat the keywords: {}",
        facts.xmp
    );
    assert!(
        !facts.display_doc_title && facts.pdf_ua_part().is_none(),
        "PDF/UA-1 is selected by a profile, not implied"
    );
    assert!(
        facts.bookmarks.iter().any(|b| b == "1 Introduction") &&
            facts
                .bookmarks
                .iter()
                .any(|b| b == "2 Publication requirements"),
        "bookmarks must carry heading numbers: {:?}",
        facts.bookmarks
    );
    assert!(
        facts.bookmarks.iter().any(|b| b == "Abstract"),
        "unnumbered front matter stays unnumbered: {:?}",
        facts.bookmarks
    );
    let second = tempfile::tempdir()?;
    let repeated = publish(&publisher, &plans[0], second.path()).await?;
    assert!(
        tokio::fs::read(only_pdf(&repeated)).await? == bytes,
        "the same pinned plan must produce byte-identical PDFs"
    );
    Ok(())
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
    let directory = publish(
        &Publisher::new(dataset)
            .with_edition(grapharch::publication::pdf_name::Edition::Default),
        &plans[2],
        out.path(),
    )
    .await?;
    let facts = PdfFacts::of(&only_pdf(&directory))?;
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
    let directory = publish(
        &Publisher::new(dataset)
            .with_edition(grapharch::publication::pdf_name::Edition::Default),
        &plans[2],
        out.path(),
    )
    .await?;
    let typst = tokio::fs::read_to_string(typst_source(
        out.path(),
        &only_pdf(&directory),
    )?)
    .await?;
    assert!(
        typst.contains("#ga-wrap(") && typst.contains("#ga-wrap-raw("),
        "long table identifiers need line-break opportunities"
    );
    assert!(
        !typst.contains('\u{200b}'),
        "break opportunities must not be characters of the identifier"
    );
    let facts = PdfFacts::of(&only_pdf(&directory))?;
    assert!(
        !facts.zero_width_text,
        "copied, searched and spoken identifiers must not contain U+200B"
    );
    Ok(())
}

#[tokio::test]
async fn keywords_with_list_separators_are_rejected() -> Result<()> {
    let (dataset, plans) =
        specification("\nex:target ga:keyword \"graphs, documents\" .\n")
            .await?;
    let out = tempfile::tempdir()?;
    let error = publish(
        &Publisher::new(dataset)
            .with_edition(grapharch::publication::pdf_name::Edition::Default),
        &plans[0],
        out.path(),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("keyword"),
        "unexpected error: {error:#}"
    );
    Ok(())
}

async fn family(renderer: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    let source = tempfile::tempdir()?;
    let corpus = source.path().join("sources");
    tokio::fs::create_dir(&corpus).await?;
    tokio::fs::write(corpus.join("ontology.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
<https://example.org/vocab/> a owl:Ontology; rdfs:label "Example vocabulary"; dct:description "An example ontology." .
<https://example.org/vocab/Thing> a owl:Class; rdfs:label "Thing"; dct:description "An example thing."; rdfs:subClassOf <https://example.org/vocab/Parent> .
<https://example.org/vocab/Parent> a owl:Class; rdfs:label "Parent"; dct:description "The parent." .
<https://example.org/vocab/part> a owl:ObjectProperty; rdfs:label "part"; dct:description "Relates a thing to its parent."; rdfs:domain <https://example.org/vocab/Thing>; rdfs:range <https://example.org/vocab/Parent> .
"#).await?;
    tokio::fs::write(corpus.join("other.ttl"), r#"
@prefix owl:<http://www.w3.org/2002/07/owl#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
<https://example.org/other/> a owl:Ontology; rdfs:label "Other vocabulary"; dct:description "A second ontology." .
<https://example.org/other/Item> a owl:Class; rdfs:label "Item"; dct:description "An item." .
"#).await?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, format!(r##"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix gov:<https://ekgf.org/ontology/artifact-governance#> .
@prefix pub:<https://ekgf.org/ontology/artifact-publication#> .
@prefix rdfs:<http://www.w3.org/2000/01/rdf-schema#> .
@prefix dct:<http://purl.org/dc/terms/> .
@prefix sh:<http://www.w3.org/ns/shacl#> .
<urn:ua:project> a ga:Project; dct:title "Accessible vocabularies"; ga:target <urn:ua:target> .
<urn:ua:scope> a ga:Scope .
<urn:ua:profile> a ga:Profile; pub:publishesFamily <urn:ua:family>; ga:pdfStandard ga:PdfUa1 .
<urn:ua:family> a gov:Family; rdfs:label "Accessible vocabularies" .
<urn:ua:target> a ga:Target; dct:title "Accessible vocabularies"; dct:description "Collection reference."; rdfs:comment "Unofficial preview."; ga:scope <urn:ua:scope>; ga:profile <urn:ua:profile>; ga:renderer ga:{renderer}; ga:documentType ga:SpecificationDocument; ga:slug "reference"; ga:destination "reference"; ga:publisher <urn:ua:publisher>; ga:publicationStatus "Documentation Preview"; ga:publicationDate "2026-10-31"^^<http://www.w3.org/2001/XMLSchema#date>; ga:thisVersion <https://example.org/reference/>; ga:latestVersion <https://example.org/reference/>; ga:license <https://opensource.org/licenses/MIT> .
<urn:ua:publisher> a gov:Authority ; <https://ekgf.org/ontology/grapharch#slug> "example"; rdfs:label "Example publisher"; ga:shortName "Example"; ga:url "https://example.org/"; ga:accentColor "#2b5f9f" .
<https://example.org/vocab/> gov:inFamily <urn:ua:family>; sh:declare [sh:prefix "example"; sh:namespace "https://example.org/vocab/"] .
<https://example.org/other/> gov:inFamily <urn:ua:family>; sh:declare [sh:prefix "other"; sh:namespace "https://example.org/other/"] .
"##)).await?;
    let built = ontology_family::collect(
        &corpus,
        &definitions,
        "urn:ua:project",
        "urn:ua:target",
        2,
    )
    .await?;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publisher = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .with_book_workers(PathBuf::from(env!("CARGO_BIN_EXE_grapharch")), 1)?;
    let directory = publish(&publisher, &built.plan, output.path()).await?;
    Ok((output, directory))
}

#[tokio::test]
async fn pdf_ua_profile_makes_every_family_pdf_conform_to_pdf_ua_1()
-> Result<()> {
    for renderer in ["SpecificationRenderer", "BookshelfRenderer"] {
        let (_output, directory) = family(renderer).await?;
        let mut pdfs = Vec::new();
        let mut files = tokio::fs::read_dir(&directory).await?;
        while let Some(file) = files.next_entry().await? {
            if file.path().extension().is_some_and(|e| e == "pdf") {
                pdfs.push(file.path());
            }
        }
        assert!(
            !pdfs.is_empty(),
            "{renderer}: at least one PDF must be generated"
        );
        for path in pdfs {
            let facts = PdfFacts::of(&path)?;
            assert_eq!(
                facts.pdf_ua_part(),
                Some("1"),
                "{renderer}: {} must identify PDF/UA-1",
                path.display()
            );
            assert!(
                facts.marked && facts.structure_tree,
                "{renderer}: PDF/UA requires a tagged structure tree"
            );
            assert!(
                facts.display_doc_title,
                "{renderer}: PDF/UA viewers must display the document title"
            );
            assert!(facts.title.is_some() && facts.language.is_some());
            assert!(
                !facts.zero_width_text,
                "{renderer}: wrapped IRIs must extract without U+200B"
            );
        }
    }
    Ok(())
}

/// The one PDF edition published in `directory`.
#[allow(dead_code)]
fn only_pdf(directory: &std::path::Path) -> std::path::PathBuf {
    let mut pdfs = std::fs::read_dir(directory)
        .expect("PDF directory")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "pdf"))
        .collect::<Vec<_>>();
    assert_eq!(
        pdfs.len(),
        1,
        "one PDF edition in {}",
        directory.display()
    );
    pdfs.remove(0)
}
