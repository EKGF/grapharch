#[path = "support/definitions.rs"]
mod definitions;

#[path = "support/family_repository.rs"]
mod family_repository;

#[path = "support/family_showcase.rs"]
mod family_showcase;

#[path = "support/dprod_repository.rs"]
mod dprod_repository;

use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        website,
    },
    std::path::Path,
};

#[tokio::test]
async fn gallery_preserves_live_showcase_artifacts_in_the_exported_graph()
-> Result<()> {
    let source = tempfile::tempdir()?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_tree(
        &repo.join("examples"),
        &source.path().join("examples"),
    )?;
    // Exercise an HTML-only publication alongside genuine multi-PDF examples.
    let authored = source.path().join("examples/showcase/project.ttl");
    let text = std::fs::read_to_string(&authored)?;
    assert_eq!(text.matches("ga:renderer \"typst\"").count(), 1);
    std::fs::write(
        &authored,
        text.replace("ga:renderer \"typst\"", "ga:renderer \"html\""),
    )?;
    let registry = source.path().join("examples/showcases.ttl");
    let text = std::fs::read_to_string(&registry)?;
    assert_eq!(
        text.matches(" ; ga:pdfPath \"review/document.pdf\" .")
            .count(),
        2
    );
    std::fs::write(
        &registry,
        text.replacen(" ; ga:pdfPath \"review/document.pdf\" .", " .", 1),
    )?;
    // The acceptance test explicitly selects a local bare repository, never the
    // network.
    let family = source.path().join("examples/ekg-ontologies");
    let repository = tempfile::tempdir()?;
    let (url, commit) = family_repository::bare_copy(
        &family.join("sources"),
        repository.path(),
    )?;
    let mut config: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(family.join("showcase.yaml"))?,
    )?;
    // This old acceptance corpus predates upstream-owned publication hints.
    // Its explicit test override is distinct from the transport-only live
    // configuration.
    config["definitions"] = "project.ttl".into();
    config["project"] = "urn:ekgf-family:project".into();
    config["target"] = "urn:ekgf-family:specification".into();
    config["source"]["url"] = url.into();
    config["source"]["ref"] = commit.into();
    std::fs::write(
        family.join("showcase.yaml"),
        serde_yaml::to_string(&config)?,
    )?;
    // Live HTTP corpora use explicit synthetic local transports in this offline
    // gallery test; their dedicated acceptance tests exercise original sources.
    let commons_repository = family_showcase::configure(
        &source.path().join("examples/omg-commons"),
        "urn:omg-commons:family",
        "https://www.omg.org/spec/Commons/Classifiers/",
    )
    .await?;
    let fibo_repository = family_showcase::configure(
        &source.path().join("examples/fibo-be"),
        "urn:fibo-be:family",
        "https://spec.edmcouncil.org/fibo/ontology/BE/LegalEntities/LegalPersons/",
    ).await?;
    let dprod_repository = tempfile::tempdir()?;
    dprod_repository::configure(
        &source.path().join("examples/dprod"),
        dprod_repository.path(),
    )?;
    let mkdocs_repository = tempfile::tempdir()?;
    let mkdocs_root = mkdocs_repository.path();
    tokio::fs::create_dir(mkdocs_root.join("docs")).await?;
    tokio::fs::write(
        mkdocs_root.join("docs/index.md"),
        "# Independent documentation\n\nOriginal public-site fixture.\n",
    )
    .await?;
    tokio::fs::write(
        mkdocs_root.join("mkdocs.yml"),
        "site_name: Independent documentation\n",
    )
    .await?;
    tokio::fs::write(
        mkdocs_root.join("LICENSE"),
        "Copyright original authors. Redistribution permitted.\n",
    )
    .await?;
    family_repository::git(mkdocs_root, &["init", "-b", "main"])?;
    family_repository::git(mkdocs_root, &["add", "."])?;
    family_repository::git(mkdocs_root, &[
        "commit",
        "-m",
        "Independent documentation fixture",
    ])?;
    let commit = family_repository::git(mkdocs_root, &["rev-parse", "HEAD"])?;
    let url = url::Url::from_directory_path(mkdocs_root).unwrap();
    tokio::fs::write(
        source
            .path()
            .join("examples/mkdocs-bootswatch/showcase.yaml"),
        format!(
            "repository: {url}\ncommit: {commit}\nconfiguration: \
             mkdocs.yml\npaths: [docs, mkdocs.yml, LICENSE]\nlicense: \
             LICENSE\n"
        ),
    )
    .await?;
    let docs = source.path().join("docs");
    std::fs::create_dir(&docs)?;
    std::fs::write(
        docs.join("grapharch.ttl"),
        definitions::edit_definitions(
            include_str!("../docs/grapharch.ttl"),
            &[
                "ontologySource",
                "navigation",
                "layout",
                "collector",
                "include",
            ],
            None,
        )?,
    )?;
    let configuration =
        tokio::fs::read_to_string(docs.join("grapharch.ttl")).await?;
    tokio::fs::write(
        docs.join("grapharch.ttl"),
        format!(
            "{configuration}\n@prefix ga: <https://ekgf.org/ontology/grapharch#> .\n@prefix dct: <http://purl.org/dc/terms/> .\n<urn:grapharch:docs:profile> ga:layout \
             \"sectioned\"; ga:navigation <urn:gallery:sections> \
             .\n<urn:gallery:sections> a ga:Navigation; dct:title \
             \"Sections\"; ga:navigationRegion \"primary\"; \
             ga:navigationEntry <urn:gallery:tab> .\n<urn:gallery:tab> a \
             ga:NavigationEntry; dct:title \"Gallery\"; ga:order 1; ga:page \
             <urn:gallery:page> .\n"
        ),
    )
    .await?;
    std::fs::write(
        docs.join("index.md"),
        "---\ntitle: Gallery\ngrapharch:\n  subject: urn:gallery:page\n  \
         showcase_gallery: true\n---\n# Showcase gallery\n",
    )?;
    let built = website::collect(
        &docs,
        &docs.join("grapharch.ttl"),
        "urn:grapharch:docs",
        "urn:grapharch:docs:website",
        4,
    )
    .await?;
    let mut placeholders = built
        .dataset
        .select(
            "PREFIX ga: <https://ekgf.org/ontology/grapharch#> SELECT ?asset \
             WHERE { GRAPH ?g { ?asset a ga:BinaryAsset, ga:Fragment } } \
             LIMIT 1"
                .into(),
        )
        .await?;
    assert!(
        placeholders.next().await?.is_none(),
        "binary artifacts must be explicit assets, not empty documentation \
         fragments"
    );
    let mut assets = built
        .dataset
        .select(format!(
            "PREFIX ga: <https://ekgf.org/ontology/grapharch#> SELECT ?asset \
             WHERE {{ GRAPH ?g {{ {} ga:asset ?asset }} }} LIMIT 1",
            built.plan.id
        ))
        .await?;
    assert!(
        assets.next().await?.is_some(),
        "the publication plan must explicitly select its assets"
    );
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    std::fs::remove_dir_all(source.path())?;
    std::fs::remove_dir_all(repository.path())?;
    std::fs::remove_dir_all(commons_repository.path())?;
    tokio::fs::remove_dir_all(fibo_repository.path()).await?;
    std::fs::remove_dir_all(dprod_repository.path())?;
    tokio::fs::remove_dir_all(mkdocs_repository.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("expected gallery publication")
    };
    let html =
        std::fs::read_to_string(publication.directory.join("index.html"))?;
    if std::env::var_os("GRAPHARCH_GALLERY_PREVIEW").is_some() {
        copy_tree(
            &publication.directory,
            &repo.join("output/gallery-review"),
        )?;
    }
    let document = scraper::Html::parse_document(&html);
    let cards = scraper::Selector::parse(".showcase-card").unwrap();
    assert_eq!(document.select(&cards).count(), 12);
    assert!(!html.contains("Open HTML"));
    assert!(!html.contains("class=\"site-navigation\""));
    assert!(html.contains("gallery-home"));
    for card in document.select(&cards) {
        let primary = scraper::Selector::parse("a.showcase-primary").unwrap();
        let links = card.select(&primary).collect::<Vec<_>>();
        assert_eq!(links.len(), 1, "one named primary card link");
        assert!(!links[0].text().collect::<String>().trim().is_empty());
        assert_eq!(
            card.select(&scraper::Selector::parse("a a").unwrap())
                .count(),
            0
        );
        assert!(links[0].value().attr("href").unwrap().ends_with(".html"));
    }

    assert_eq!(
        document
            .select(
                &scraper::Selector::parse("iframe[sandbox][loading=lazy]")
                    .unwrap()
            )
            .count(),
        12
    );
    let mut detail_html = String::new();
    for link in
        document.select(&scraper::Selector::parse(".showcase-primary").unwrap())
    {
        let route = link.value().attr("href").unwrap();
        assert!(route.contains("showcase-"));
        let detail =
            tokio::fs::read_to_string(publication.directory.join(route))
                .await?;
        let parsed = scraper::Html::parse_document(&detail);
        let active = parsed
            .select(
                &scraper::Selector::parse(
                    ".primary-navigation .active-section",
                )
                .unwrap(),
            )
            .next()
            .expect("detail keeps its gallery section");
        assert_eq!(active.text().collect::<String>(), "Gallery");
        assert!(!detail.contains("class=\"site-navigation\""));
        detail_html.push_str(&detail);
    }
    assert!(!html.contains("Download KG"));
    assert!(!html.contains("All publication outputs"));
    let details = scraper::Html::parse_document(&detail_html);
    for link in details.select(
        &scraper::Selector::parse(".showcase-detail a[data-artifact]").unwrap(),
    ) {
        let path = publication
            .directory
            .join(link.value().attr("href").unwrap());
        assert!(
            path.is_file(),
            "missing artifact {}",
            path.display()
        );
        if path.extension().is_some_and(|ext| ext == "pdf") {
            assert!(std::fs::read(path)?.starts_with(b"%PDF-"));
        }
    }
    let html_only = details
        .select(
            &scraper::Selector::parse(
                "[data-showcase='urn:grapharch:showcase:authored']",
            )
            .unwrap(),
        )
        .next()
        .unwrap();
    assert_eq!(
        html_only.value().attr("data-output"),
        Some("HTML|KG")
    );
    assert!(!html_only.html().contains(".pdf"));
    for card in
        details.select(&scraper::Selector::parse(".showcase-detail").unwrap())
    {
        let actions =
            scraper::Selector::parse(".showcase-actions a[href$='.pdf']")
                .unwrap();
        let outputs =
            scraper::Selector::parse("details a[href$='.pdf']").unwrap();
        let visible = card.select(&actions).collect::<Vec<_>>();
        let all = card.select(&outputs).collect::<Vec<_>>();
        assert_eq!(
            visible.len(),
            all.len(),
            "every PDF edition must be visible on {}",
            card.value().attr("data-showcase").unwrap()
        );
        if all.len() > 1 {
            for link in visible {
                assert_ne!(
                    link.text().collect::<String>(),
                    "PDF",
                    "multiple PDFs need distinct titles"
                );
            }
        }
    }
    assert!(html.contains("id=\"showcase-search\""));
    assert!(html.contains("aria-live=\"polite\""));
    assert!(html.contains("data-gallery-filter=\"input\""));
    assert!(html.contains("data-gallery-filter=\"capability\""));
    assert!(html.contains("data-gallery-filter=\"output\""));
    assert!(html.contains("No showcases match"));
    assert!(html.contains("data-input=\"OWL|SHACL\""));
    assert!(detail_html.contains("Source and credits"));
    assert!(detail_html.contains("https://github.com/EKGF/dprod"));
    assert!(html.contains("artifacts/dprod/"));
    assert!(html.contains("DPROD: native ontology and SHACL"));
    assert!(!html.contains("artifacts/rdf11/"));
    assert!(!html.contains("W3C RDF 1.1 N-Triples"));
    assert!(html.contains("1 known issue in the source document"));
    assert!(!html.contains("expected source diagnostic"));
    Ok(())
}

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
