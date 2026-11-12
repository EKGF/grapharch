//! House styles are IRI-identified theme resources with declarative tokens.
use {
    anyhow::Result,
    grapharch::publication::{
        render::{PublishOutcome, Publisher},
        showcases,
    },
};

const LOGO: &str = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI0MCIgaGVpZ2h0PSIxMCI+PHJlY3Qgd2lkdGg9IjQwIiBoZWlnaHQ9IjEwIiBmaWxsPSIjMDA1YTljIi8+PC9zdmc+";

/// A consumer house style refining the catalogue specification theme.
fn house(extra: &str) -> String {
    format!(
        r##"
@prefix ht: <urn:example:house:> .
ht:theme a ga:Theme ; ga:baseTheme ga:SpecificationTheme ;
    ga:paper "us-letter" ; ga:marginLeft "0.79in" ; ga:bodyFont "Liberation Serif" ;
    ga:justify false ; ga:hyphenate false ; ga:headingNumberGap "0.5in" ;
    ga:tableHeaderFill "#4e80bc" ; ga:tableHeaderText "#ffffff" ;
    ga:codeFont "Liberation Mono" ; ga:codeFill "none" ; ga:codeRuleColor "none" ;
    ga:linkColor "#0000ff" ; ga:linkUnderline true ;
    ga:footerShowsText true ; ga:outerPageNumber true ; ga:pageOfTotal false ;
    ga:frontNumbering "i" ; ga:bodyNumbering "1" ;
    ga:tocTitle "Table of Contents" ; ga:tocTitleAlign "center" ; ga:contentsDepth 2 ;
    ga:annexLabel "Annex {{letter}}: {{title}}" ; ga:annexNormativity true ;
    ga:annexCentered true ; ga:annexRectoStart true ;
    ga:blankPageText "This page intentionally left blank." ;
    ga:coverBlock ht:date, ht:logo, ht:title, ht:version, ht:rule, ht:rows .
ht:date a ga:CoverBlock ; ga:order 1 ; ga:coverElement ga:CoverDate ; ga:dateFormat ga:MonthYearDate ; ga:fontSize "12pt" ; ga:alignment "right" .
ht:logo a ga:CoverBlock ; ga:order 2 ; ga:coverElement ga:CoverLogo ; ga:height "0.55in" .
ht:title a ga:CoverBlock ; ga:order 3 ; ga:coverElement ga:CoverTitle ; ga:fontSize "24pt" ; ga:fontWeight "bold" .
ht:version a ga:CoverBlock ; ga:order 4 ; ga:coverElement ga:CoverVersion ; ga:textPattern "Version {{version}}" ; ga:fontSize "18pt" ; ga:italic true .
ht:rule a ga:CoverBlock ; ga:order 5 ; ga:coverElement ga:CoverRule ; ga:stroke "0.6pt" ; ga:color "#000000" .
ht:rows a ga:CoverBlock ; ga:order 6 ; ga:coverElement ga:CoverRows ; ga:fontSize "12pt" ; ga:fontWeight "bold" .
ex:profile ga:theme ht:theme .
ex:target ga:releaseVersion "1.3" ; ga:documentNumber "formal/26-06-05" ;
    ga:footerText "Example Specification (EX), 1.3" ;
    ga:metadataRow ex:number-row, ex:latest-row .
ex:number-row dct:title "Example Document Number:" ; ga:field "documentNumber" ; ga:order 1 .
ex:latest-row dct:title "Normative reference:" ; ga:field "latestVersion" ; ga:order 2 .
ex:changes ga:normativity "informative" .
<urn:uuid:6c1408aa-039c-42f9-8dee-9c3f0009d61a> ga:logo "data:image/svg+xml;base64,{LOGO}" .
{extra}
"##
    )
}

async fn render(definitions: String) -> Result<(String, tempfile::TempDir)> {
    let source = tempfile::tempdir()?;
    let path = source.path().join("project.ttl");
    // The specification example declares a PNG logo; the house theme supplies
    // its own SVG logo for the same authority.
    let base = include_str!("../examples/specification/project.ttl")
        .replacen(
            "ga:url \"https://ekgf.org/\" ; ga:logo",
            "ga:url \"https://ekgf.org/\" ; ga:unusedLogo",
            1,
        )
        .replacen(
            "grapharch#SpecificationRenderer",
            "grapharch#TypstRenderer",
            1,
        );
    tokio::fs::write(&path, format!("{base}\n{definitions}")).await?;
    let (dataset, plans) = showcases::specification(&path).await?;
    source.close()?;
    let out = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(dataset)
        .render(&plans[0], out.path())
        .await?
    else {
        panic!("publication required")
    };
    let typst =
        tokio::fs::read_to_string(publication.directory.join("main.typ"))
            .await?;
    assert!(
        tokio::fs::read(publication.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok((typst, out))
}

#[tokio::test]
async fn consumer_theme_tokens_override_the_catalogue_theme() -> Result<()> {
    let (typst, _out) = render(house("")).await?;
    assert!(typst.contains("// Theme: urn:example:house:theme"));
    for token in [
        "  paper: \"us-letter\",",
        "  margin_left: 0.79in,",
        "  margin_top: 25mm,",
        "  body_font: \"Liberation Serif\",",
        "  justify: false,",
        "  heading_number_gap: 0.5in,",
        "  table_header_fill: rgb(\"#4e80bc\"),",
        "  code_fill: none,",
        "  link_color: rgb(\"#0000ff\"),",
        "  outer_page_number: true,",
    ] {
        assert!(typst.contains(token), "missing {token}");
    }
    assert!(!typst.contains("omg") && !typst.contains("ga-style"));
    assert!(
        typst.contains(
            "#let ga-footer-text = \"Example Specification (EX), 1.3\""
        )
    );
    assert!(typst.contains("#outline(title: none, depth: 2)"));
    assert!(typst.contains("[#text(\"Table of Contents\")]"));
    Ok(())
}

#[tokio::test]
async fn declarative_cover_places_logo_date_version_and_labelled_rows()
-> Result<()> {
    let (typst, out) = render(house("")).await?;
    let start = typst.find("#set text(font: ga-theme.cover_font)").unwrap();
    let cover = &typst[start..typst.find("#outline(").unwrap()];
    if let Ok(path) = std::env::var("GRAPHARCH_THEME_REVIEW") {
        std::fs::write(path, &typst)?;
    }
    let date = cover
        .find("#text(\"October 2026\")")
        .expect("month-year date");
    let logo = cover
        .find("#image(\"cover-logo.svg\", height: 0.55in")
        .expect("logo");
    let title = cover.find("Documentation Publication Model 1.0").unwrap();
    let version = cover.find("#text(\"Version 1.3\")").expect("version line");
    let rows = cover
        .find("#text(\"Example Document Number:\")")
        .expect("row label");
    assert!(date < logo && logo < title && title < version && version < rows);
    assert!(cover.contains("#text(\"formal/26-06-05\")"));
    assert!(cover.contains("[#text(\"Normative reference:\")], [#link(\"https://example.org/spec/\")"));
    assert!(!cover.contains("Document identifier:"));
    let logo = std::fs::read_dir(out.path())?
        .flatten()
        .find_map(|entry| {
            let path = entry.path().join("cover-logo.svg");
            path.exists().then_some(path)
        })
        .expect("written cover logo");
    assert!(std::fs::read_to_string(logo)?.starts_with("<svg"));
    Ok(())
}

#[tokio::test]
async fn annexes_use_label_pattern_normativity_and_recto_start() -> Result<()> {
    let (typst, _out) = render(house("")).await?;
    assert!(typst.contains(
        "#ga-blank-page[#text(\"This page intentionally left blank.\")]"
    ));
    assert!(typst.contains("[#text(\"Annex A: Change history\")]"));
    assert!(typst.contains("#text(\"(informative)\")"));
    assert!(
        typst.contains("#align(center)[#heading(level: 1, numbering: none)")
    );
    let body = typst
        .find("#counter(page).update(1)\n#set page(numbering: \"1\")")
        .expect("arabic body");
    assert!(typst.find("#set page(numbering: \"i\")").unwrap() < body);
    Ok(())
}

#[tokio::test]
async fn annex_without_normativity_fails_fast() -> Result<()> {
    let definitions =
        house("").replace("ex:changes ga:normativity \"informative\" .", "");
    let error = render(definitions)
        .await
        .err()
        .expect("missing normativity");
    assert!(format!("{error:#}").contains("requires ga:normativity"));
    Ok(())
}

#[tokio::test]
async fn invalid_and_removed_theme_values_fail_fast() -> Result<()> {
    for (definitions, message) in [
        (
            house("ht:theme ga:bodySize \"10px\" ."),
            "requires a unit",
        ),
        (
            house("ht:theme ga:captionFont \"Arial\\\"); x(\" ."),
            "must name a font family",
        ),
        (
            "<urn:example:a> a ga:Theme ; ga:baseTheme <urn:example:b> \
             .\n<urn:example:b> a ga:Theme ; ga:baseTheme <urn:example:a> \
             .\nex:profile ga:theme <urn:example:a> ."
                .to_owned(),
            "cyclic ga:baseTheme chain",
        ),
        (
            "ex:profile ga:style \"omg\" .".to_owned(),
            "removed presentation style",
        ),
        (
            "<urn:example:lonely> a ga:Theme ; ga:paper \"a4\" .\nex:profile \
             ga:theme <urn:example:lonely> ."
                .to_owned(),
            "must refine a GraphArch catalogue theme",
        ),
        (
            house("ht:theme ga:pageColour \"#ffffff\" ."),
            "unsupported token ga:pageColour",
        ),
    ] {
        let error = render(definitions).await.err().expect("invalid theme");
        assert!(
            format!("{error:#}").contains(message),
            "{error:#} lacks {message}"
        );
    }
    Ok(())
}
