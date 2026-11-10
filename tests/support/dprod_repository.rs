use {
    super::{definitions, family_repository},
    anyhow::Result,
    std::path::Path,
};

/// Select a tiny local Git source while exercising the real remote collector.
pub fn configure(example: &Path, repository: &Path) -> Result<()> {
    let corpus = tempfile::tempdir()?;
    for directory in ["publication", "ontology/dprod", "respec"] {
        std::fs::create_dir_all(corpus.path().join(directory))?;
    }
    for (source, target) in [
        (
            "sources/ontology.ttl",
            "ontology/dprod/dprod-ontology.ttl",
        ),
        (
            "sources/shapes.ttl",
            "ontology/dprod/dprod-shapes.ttl",
        ),
        ("abstract.md", "publication/abstract.md"),
        ("status.md", "publication/status.md"),
    ] {
        std::fs::copy(example.join(source), corpus.path().join(target))?;
    }
    std::fs::write(
        corpus.path().join("README.md"),
        "# Synthetic gallery overview\n\nOffline Git collection fixture.\n",
    )?;
    std::fs::write(
        corpus.path().join("example.md"),
        "# Synthetic dataset example\n\nA local example.\n",
    )?;
    let mut definitions = std::fs::read_to_string(example.join("project.ttl"))?;
    for subject in [
        "urn:dprod-showcase:project",
        "urn:dprod-showcase:specification",
    ] {
        definitions = definitions::remove_property(
            &definitions,
            subject,
            "http://purl.org/dc/terms/title",
        )?;
    }
    definitions = definitions::edit_quads(&definitions, |quads| {
        quads.retain(|quad| {
            quad.predicate.as_str() !=
                "https://ekgf.org/ontology/grapharch#license"
        });
        Ok(())
    })?;
    definitions = definitions::edit_quads(&definitions, |quads| {
        for quad in quads {
            if quad.subject.to_string() == "<urn:dprod-showcase:pdf>" &&
                quad.predicate.as_str() ==
                    "https://ekgf.org/ontology/grapharch#destination"
            {
                quad.object =
                    oxrdf::Literal::new_simple_literal("pdf/ekgf").into();
            }
        }
        Ok(())
    })?;
    definitions.push_str(
        "\n@prefix ex: <urn:dprod-showcase:> . @prefix ga: <https://ekgf.org/ontology/grapharch#> . @prefix dct: <http://purl.org/dc/terms/> .\nex:project ga:collector ex:c1,ex:c2,ex:c3,ex:c4,ex:c5,ex:c6 .\n",
    );
    for (id, path, subject) in [
        (
            "c1",
            "ontology/dprod/dprod-ontology.ttl",
            "https://www.omg.org/spec/DPROD/dprod/",
        ),
        (
            "c2",
            "ontology/dprod/dprod-shapes.ttl",
            "https://www.omg.org/spec/DPROD/shapes/",
        ),
    ] {
        definitions.push_str(&format!(
            "ex:{id} a ga:CollectorUse ; ga:collectorType ga:RdfFileCollector \
             ; ga:sourcePath \"{path}\" ; ga:source \"{path}\" ; ga:subject \
             <{subject}> .\n"
        ));
    }
    for (id, path, key, title, route) in [
        (
            "c3",
            "publication/abstract.md",
            "abstract",
            "Abstract",
            None,
        ),
        (
            "c4",
            "publication/status.md",
            "status",
            "Status of This Document",
            None,
        ),
        (
            "c5",
            "README.md",
            "overview",
            "Development documentation",
            Some(""),
        ),
        (
            "c6",
            "example.md",
            "example",
            "Dataset example",
            Some("guide/"),
        ),
    ] {
        definitions.push_str(&format!(
            "ex:{id} a ga:CollectorUse ; ga:collectorType ga:{} ; \
             ga:sourcePath \"{path}\" ; ga:source \"{path}\" ; ga:fragment \
             ex:{key} ; dct:identifier \"{key}\"",
            if route.is_some() {
                "MarkdownPageCollector"
            } else {
                "MarkdownFileCollector"
            }
        ));
        if let Some(route) = route {
            definitions.push_str(&format!(
                " ; dct:title \"{title}\" ; ga:route \"{route}\""
            ));
        }
        definitions.push_str(" .\n");
    }
    std::fs::write(
        corpus.path().join("formal.md"),
        "# Formal scope\n\nSOURCE OWNED FORMAL SCOPE\n",
    )?;
    definitions.push_str(r#"
ex:project ga:collector ex:formal-source .
ex:formal-source a ga:CollectorUse ; ga:collectorType ga:MarkdownFileCollector ; ga:sourcePath "formal.md" ; ga:source "formal.md" ; ga:fragment ex:formal .
ex:profile ga:requiredSectionRole "scope" .
ex:pdf-profile ga:requiredSectionRole "scope" .
"#);
    for profile in ["profile", "pdf-profile"] {
        for (key, title, kind, role, order) in [
            ("abstract", "Abstract", "abstract", "abstract", 0),
            (
                "status",
                "Status of This Document",
                "status",
                "status",
                1,
            ),
            ("formal", "Formal scope", "section", "scope", 10),
            (
                "overview",
                "Vocabulary reference",
                "section",
                "reference",
                20,
            ),
            (
                "example",
                "Dataset example",
                "section",
                "example",
                100,
            ),
        ] {
            definitions.push_str(&format!(r#"
ex:{profile} ga:placement <urn:dprod-showcase:{profile}:{key}> .
<urn:dprod-showcase:{profile}:{key}> a ga:Placement ; ga:component <urn:dprod-showcase:component:{key}> ; ga:sectionKind "{kind}" ; ga:sectionRole "{role}" ; ga:order {order} .
<urn:dprod-showcase:component:{key}> a ga:Component ; dct:title "{title}" ; ga:fragment ex:{key} .
"#));
        }
    }
    std::fs::write(
        corpus.path().join("publication/standards.ttl"),
        definitions,
    )?;
    std::fs::write(
        corpus.path().join("respec/template.html"),
        r#"<details class="preamble-section"><summary>Preamble</summary><p>Synthetic test legal terms.</p></details>"#,
    )?;
    let (url, commit) =
        family_repository::bare_copy(corpus.path(), repository)?;
    std::fs::write(
        example.join("live.yaml"),
        format!(
            "repository: {url}\nref: {commit}\ndefinitions: \
             publication/standards.ttl\n"
        ),
    )?;
    Ok(())
}
