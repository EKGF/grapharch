use super::*;

const BASE: &str = "https://ekgf.github.io/dprod/spec/develop/";

#[test]
fn publication_urls_are_absolute_http_directories() -> Result<()> {
    assert_eq!(PublicationUrl::try_new(BASE)?.as_str(), BASE);
    assert_eq!(
        PublicationUrl::try_new("http://localhost:8000/")?.as_str(),
        "http://localhost:8000/"
    );
    for value in [
        "https://ekgf.github.io/dprod/spec/develop",
        "ftp://ekgf.github.io/dprod/",
        "file:///srv/dprod/",
        "https://reader:secret@ekgf.github.io/dprod/",
        "https://ekgf.github.io/dprod/?preview=1",
        "https://ekgf.github.io/dprod/#top",
        "dprod/spec/develop/",
        "",
    ] {
        let error =
            PublicationUrl::try_new(value).expect_err(value).to_string();
        assert!(error.contains(REQUIREMENT), "{value}: {error}");
    }
    Ok(())
}

#[test]
fn versions_name_each_renderer_entry_point_beneath_the_destination()
-> Result<()> {
    let base = PublicationUrl::try_new(BASE)?;
    for (destination, renderer, expected) in [
        (
            "specification",
            Renderer::Specification,
            "specification/",
        ),
        ("website", Renderer::Website, "website/"),
        ("reader", Renderer::Html, "reader/"),
        ("pages", Renderer::SpecificationPages, "pages/"),
        ("books", Renderer::Bookshelf, "books/"),
        (
            "pdf/ekgf",
            Renderer::Typst,
            "pdf/ekgf/document.pdf",
        ),
        (
            "review copy",
            Renderer::Typst,
            "review%20copy/document.pdf",
        ),
    ] {
        assert_eq!(
            base.this_version(destination, renderer)?.as_str(),
            format!("{BASE}{expected}"),
            "{destination} with {renderer:?}"
        );
    }
    for destination in
        ["", "/pdf", "pdf/", "pdf//omg", "./pdf", "../pdf", "pdf/.."]
    {
        let error = base
            .this_version(destination, Renderer::Typst)
            .expect_err(destination)
            .to_string();
        assert!(
            error.contains("relative directory"),
            "{destination}: {error}"
        );
    }
    Ok(())
}
