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
        oxrdf::{NamedNode, NamedOrBlankNode, Term},
        oxrdfio::{RdfFormat, RdfParser, RdfSerializer},
    };
    const NAMESPACE: &str = "https://ekgf.org/ontology/grapharch#";
    let removed = removed
        .iter()
        .map(|name| NamedNode::new(format!("{NAMESPACE}{name}")))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut seen = vec![false; removed.len()];
    let repository = repository
        .map(
            |(subject, url)| -> Result<(NamedOrBlankNode, Term)> {
                Ok((
                    NamedNode::new(subject)?.into(),
                    NamedNode::new(url)?.into(),
                ))
            },
        )
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

/// Edit fixture statements as RDF, retaining prefix declarations for appended
/// test data.
pub fn edit_quads(
    source: &str,
    edit: impl FnOnce(&mut Vec<oxrdf::Quad>) -> Result<()>,
) -> Result<String> {
    use oxrdfio::{RdfFormat, RdfParser, RdfSerializer};
    let mut parser =
        RdfParser::from_format(RdfFormat::Turtle).for_slice(source.as_bytes());
    let mut quads = parser
        .by_ref()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle);
    let mut prefixes = parser.prefixes().collect::<Vec<_>>();
    prefixes.sort_unstable();
    for (prefix, iri) in prefixes {
        serializer = serializer.with_prefix(prefix, iri)?;
    }
    edit(&mut quads)?;
    let mut output = serializer.for_writer(Vec::new());
    for quad in quads {
        output.serialize_quad(&quad)?;
    }
    Ok(String::from_utf8(output.finish()?)?)
}

pub fn iri(value: &str) -> Result<oxrdf::Term> {
    Ok(oxrdf::NamedNode::new(value)?.into())
}
pub fn literal(value: &str) -> oxrdf::Term {
    oxrdf::Literal::new_simple_literal(value).into()
}

/// Set one property on an existing named fixture subject.
pub fn set_property(
    source: &str,
    subject: &str,
    predicate: &str,
    object: oxrdf::Term,
) -> Result<String> {
    use {
        anyhow::ensure,
        oxrdf::{GraphName, NamedNode},
    };
    let subject: oxrdf::NamedOrBlankNode = NamedNode::new(subject)?.into();
    let predicate = NamedNode::new(predicate)?;
    edit_quads(source, move |quads| {
        ensure!(
            quads.iter().any(|quad| quad.subject == subject),
            "fixture subject is absent: {subject}"
        );
        quads.retain(|quad| {
            quad.subject != subject || quad.predicate != predicate
        });
        quads.push(oxrdf::Quad::new(
            subject,
            predicate,
            object,
            GraphName::DefaultGraph,
        ));
        Ok(())
    })
}

pub fn remove_property(
    source: &str,
    subject: &str,
    predicate: &str,
) -> Result<String> {
    use {anyhow::ensure, oxrdf::NamedNode};
    let subject: oxrdf::NamedOrBlankNode = NamedNode::new(subject)?.into();
    let predicate = NamedNode::new(predicate)?;
    edit_quads(source, move |quads| {
        let before = quads.len();
        quads.retain(|quad| {
            quad.subject != subject || quad.predicate != predicate
        });
        ensure!(
            quads.len() < before,
            "fixture property is absent: {subject} {predicate}"
        );
        Ok(())
    })
}

#[test]
fn structural_fixture_edits_preserve_prefixes_and_literal_contents_across_layouts()
-> Result<()> {
    use oxrdfio::{RdfFormat, RdfParser};
    let source = "@prefix ga:<https://ekgf.org/ontology/grapharch#> \
                  .\n@prefix ex:<urn:example:> .\nex:target a ga:Target; \
                  ga:destination \"old\";ga:body \"ga:destination old\" .\n";
    for source in [
        source.to_owned(),
        source.replace('\n', "\r\n"),
        source.replace("; ", ";\n"),
    ] {
        let unchanged_body =
            replace_values(&source, "does not occur", "unused")?;
        assert!(unchanged_body.contains("ga:destination old"));
        let edited = set_property(
            &source,
            "urn:example:target",
            "https://ekgf.org/ontology/grapharch#destination",
            literal("new"),
        )?;
        let edited = set_property(
            &edited,
            "urn:example:target",
            "https://ekgf.org/ontology/grapharch#reference",
            iri("urn:example:reference")?,
        )?;
        let edited = remove_property(
            &edited,
            "urn:example:target",
            "https://ekgf.org/ontology/grapharch#body",
        )?;
        let extended = format!("{edited}\nex:extra a ga:Target .");
        let quads = RdfParser::from_format(RdfFormat::Turtle)
            .for_slice(extended.as_bytes())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert!(quads.iter().any(|quad| quad.object == literal("new")));
        assert!(!quads.iter().any(|quad| {
            quad.predicate.as_str() ==
                "https://ekgf.org/ontology/grapharch#body"
        }));
    }
    Ok(())
}

/// Replace fixture object values without rewriting Turtle tokens or datatype
/// syntax.
pub fn replace_values(
    source: &str,
    before: &str,
    after: &str,
) -> Result<String> {
    use oxrdf::{Literal, NamedNode, Term};
    edit_quads(source, |quads| {
        for quad in quads {
            match &quad.object {
                Term::Literal(value) if value.value().contains(before) => {
                    let text = value.value().replace(before, after);
                    quad.object = if let Some(language) = value.language() {
                        Literal::new_language_tagged_literal(text, language)?
                            .into()
                    } else {
                        Literal::new_typed_literal(
                            text,
                            value.datatype().into_owned(),
                        )
                        .into()
                    };
                },
                Term::NamedNode(value) if value.as_str().contains(before) => {
                    quad.object =
                        NamedNode::new(value.as_str().replace(before, after))?
                            .into();
                },
                _ => {},
            }
        }
        Ok(())
    })
}
