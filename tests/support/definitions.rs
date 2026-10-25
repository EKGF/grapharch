use anyhow::Result;

/// Prepare a reduced publication fixture without depending on its source
/// layout.
pub fn edit_definitions(
    source: &str,
    removed: &[&str],
    repository: Option<(&str, &str)>,
) -> Result<String> {
    use {
        anyhow::ensure,
        oxrdf::{NamedNode, Subject, Term},
        oxrdfio::{RdfFormat, RdfParser, RdfSerializer},
    };
    const NAMESPACE: &str = "https://ekgf.org/ontology/grapharch#";
    let removed = removed
        .iter()
        .map(|name| NamedNode::new(format!("{NAMESPACE}{name}")))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut seen = vec![false; removed.len()];
    let repository = repository
        .map(|(subject, url)| -> Result<(Subject, Term)> {
            Ok((
                NamedNode::new(subject)?.into(),
                NamedNode::new(url)?.into(),
            ))
        })
        .transpose()?;
    let mut changed_repository = repository.is_none();
    let mut serializer =
        RdfSerializer::from_format(RdfFormat::Turtle).for_writer(Vec::new());
    for quad in
        RdfParser::from_format(RdfFormat::Turtle).for_slice(source.as_bytes())
    {
        let mut quad = quad?;
        if let Some(index) = removed
            .iter()
            .position(|predicate| predicate == &quad.predicate)
        {
            seen[index] = true;
            continue;
        }
        if quad.predicate.as_str() ==
            "https://ekgf.org/ontology/grapharch#repository" &&
            let Some((subject, replacement)) = &repository &&
            &quad.subject == subject
        {
            quad.object = replacement.clone();
            changed_repository = true;
        }
        serializer.serialize_quad(&quad)?;
    }
    ensure!(
        seen.iter().all(|found| *found),
        "a requested fixture source predicate is absent"
    );
    ensure!(
        changed_repository,
        "the fixture project has no repository statement to replace"
    );
    Ok(String::from_utf8(serializer.finish()?)?)
}

#[test]
fn fixture_edits_ignore_line_ending_and_layout_differences() -> Result<()> {
    use oxrdfio::{RdfFormat, RdfParser};
    let source = include_str!("../../docs/grapharch.ttl").replace("\r\n", "\n");
    for input in [
        source.clone(),
        source.replace('\n', "\r\n"),
        source.replace("    ga:", "ga:"),
    ] {
        let output = edit_definitions(
            &input,
            &["ontologySource", "showcasesSource"],
            None,
        )?;
        let quads = RdfParser::from_format(RdfFormat::Turtle)
            .for_slice(output.as_bytes())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert!(
            !quads.iter().any(|quad| {
                matches!(
                    quad.predicate.as_str(),
                    "https://ekgf.org/ontology/grapharch#ontologySource" |
                        "https://ekgf.org/ontology/grapharch#showcasesSource"
                )
            }),
            "fixture source hints must be removed regardless of newline \
             encoding"
        );
        assert!(quads.iter().any(|quad| {
            quad.subject.to_string() == "<urn:grapharch:docs>" &&
                quad.predicate.as_str() ==
                    "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
        }));
    }
    Ok(())
}

#[test]
fn repository_edit_preserves_other_subjects_and_literal_contents() -> Result<()>
{
    use oxrdfio::{RdfFormat, RdfParser};
    let source = r#"@prefix ga:<https://ekgf.org/ontology/grapharch#> .
<urn:grapharch:docs> ga:repository <https://github.com/EKGF/grapharch> ; ga:body "https://github.com/EKGF/grapharch" .
<urn:unrelated> ga:repository <https://github.com/EKGF/grapharch> ."#;
    let output = edit_definitions(
        source,
        &[],
        Some(("urn:grapharch:docs", "https://example.org/repo")),
    )?;
    let quads = RdfParser::from_format(RdfFormat::Turtle)
        .for_slice(output.as_bytes())
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert!(quads.iter().any(|quad| {
        quad.subject.to_string() == "<urn:unrelated>" &&
            quad.object.to_string() == "<https://github.com/EKGF/grapharch>"
    }));
    assert!(quads.iter().any(|quad| {
        quad.object.to_string() == "\"https://github.com/EKGF/grapharch\""
    }));
    Ok(())
}
