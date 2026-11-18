//! BCP 14 keywords are presented only when a target declares the
//! conformance language, in HTML and in the Typst source of its PDF.
use {
    anyhow::Result,
    grapharch::publication::{
        assembly::Assembler,
        dataset::Dataset,
        plan::TargetPlanner,
        render::{PublishOutcome, Publisher},
        website,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
};

const GUIDE: &str = "---\ntitle: Guide\ngrapharch:\n  fragment: \
                     urn:guide\n---\n# Guide MUST\n\nA client MUST NOT retry, \
                     MAY cache and SHOULD\nNOT guess; it must not fail. \
                     `MUST` is code.\n\n```text\nSHALL\n```\n";

/// Published outputs of one website and one PDF target.
struct Rendered {
    html:  String,
    css:   String,
    typst: String,
}

async fn render(declaration: &str) -> Result<Rendered> {
    let source = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let definitions = source.path().join("project.ttl");
    tokio::fs::write(&definitions, format!(r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#>.
<urn:p> a ga:Project;ga:target <urn:web>,<urn:pdf>.
<urn:web> a ga:Target;ga:scope <urn:scope>;ga:profile <urn:profile>;ga:renderer ga:WebsiteRenderer;ga:destination "site"{declaration}.
<urn:scope> a ga:Scope. <urn:profile> a ga:Profile.
<urn:authority> a <https://ekgf.org/ontology/artifact-governance#Authority> ; <http://www.w3.org/2000/01/rdf-schema#label> "Example authority"@en ; ga:slug "example" .
<urn:pdf> a ga:Target;ga:scope <urn:pdfscope>;ga:profile <urn:pdfprofile>;ga:renderer ga:TypstRenderer;ga:documentType ga:SpecificationDocument;ga:slug "pdf";ga:publisher <urn:authority>;ga:destination "pdf"{declaration}.
<urn:pdfscope> a ga:Scope;ga:include <urn:guide>.
<urn:pdfprofile> a ga:Profile;ga:placement <urn:placement>.
<urn:placement> a ga:Placement;ga:component <urn:component>;ga:order 1.
<urn:component> a ga:Component;ga:fragment <urn:guide>.
"#)).await?;
    tokio::fs::write(source.path().join("index.md"), GUIDE).await?;
    let built =
        website::collect(source.path(), &definitions, "urn:p", "urn:web", 2)
            .await?;
    let (writer, worker) =
        GraphWriter::spawn(built.dataset.clone(), 4, 1024 * 1024)?;
    let revision = Assembler::new(built.dataset.clone())
        .assemble("urn:p")
        .await?;
    let pdf = TargetPlanner::new(built.dataset.clone(), revision)
        .resolve("urn:p", &NamedNode::new("urn:pdf")?, &writer)
        .await?;
    drop(writer);
    worker.await??;
    // Render from the exported KG after removing the sources.
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    source.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let publisher = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default);
    let PublishOutcome::Published(site) =
        publisher.render(&built.plan, output.path()).await?
    else {
        panic!("site required")
    };
    let PublishOutcome::Published(book) =
        publisher.render(&pdf, output.path()).await?
    else {
        panic!("PDF required")
    };
    assert!(book.pdf().expect("published PDF edition").exists());
    Ok(Rendered {
        html:  tokio::fs::read_to_string(site.directory.join("index.html"))
            .await?,
        css:   tokio::fs::read_to_string(site.directory.join("style.css"))
            .await?,
        typst: tokio::fs::read_to_string(book.typst_source()).await?,
    })
}

#[tokio::test]
async fn declared_bcp14_keywords_are_styled_in_html_and_pdf() -> Result<()> {
    let rendered = render(";ga:conformanceLanguage ga:Bcp14").await?;
    let html = &rendered.html;
    assert_eq!(
        html.matches("<em class=\"rfc2119\">").count(),
        3,
        "{html}"
    );
    for keyword in ["MUST NOT", "MAY", "SHOULD\nNOT"] {
        assert!(
            html.contains(&format!("<em class=\"rfc2119\">{keyword}</em>")),
            "{keyword}: {html}"
        );
    }
    assert!(
        html.contains("<code>MUST</code> is code"),
        "{html}"
    );
    assert!(html.contains("it must not fail"), "{html}");
    assert!(
        rendered
            .css
            .contains(".rfc2119{font-style:italic;font-weight:700}"),
        "the reader theme's keyword style applies to the website"
    );
    let typst = &rendered.typst;
    assert!(
        typst.contains("#{ga-normative[MUST NOT]}"),
        "{typst}"
    );
    assert!(
        typst.contains("#{ga-normative[SHOULD NOT]}"),
        "{typst}"
    );
    assert_eq!(
        typst.matches("#{ga-normative[").count(),
        3,
        "{typst}"
    );
    assert!(
        typst.contains("normative_keyword_style: \"bold-italic\""),
        "the PDF uses its reader theme's keyword style"
    );
    Ok(())
}

#[tokio::test]
async fn without_a_declaration_keywords_are_ordinary_text() -> Result<()> {
    let rendered = render("").await?;
    assert!(
        !rendered.html.contains("rfc2119"),
        "{}",
        rendered.html
    );
    assert!(!rendered.css.contains(".rfc2119"));
    assert!(!rendered.typst.contains("#{ga-normative["));
    Ok(())
}

#[tokio::test]
async fn an_unknown_conformance_language_is_rejected() {
    let error = render(";ga:conformanceLanguage <urn:example:Iso2119>")
        .await
        .err()
        .expect("an undeclared conformance language must fail");
    assert!(
        format!("{error:#}").contains("ConformanceLanguage"),
        "{error:#}"
    );
}
