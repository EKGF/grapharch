use {
    super::{Property, SH, Shape},
    crate::publication::document::annotated_term::escape,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, HashSet},
};

pub(super) async fn append(
    shapes: &[Shape],
    class: &NamedNode,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
    body: &mut String,
    renderer: &crate::publication::document::plantuml::LocalRenderer,
) -> Result<()> {
    let mut output = String::from("\n\n**SHACL validation views**\n");
    for shape in shapes {
        let projection =
            super::diagram::generate(shape, class, prefixes, renderer).await?;
        let reference = resource(&shape.id, prefixes, documented);
        output.push_str(&format!(
            "\n\n**Validation shape: {reference}**\n\n[Full shape \
             definition](<{}>)\n",
            shape.id.as_str()
        ));
        output.push_str(&format!(
            "\n\n```{}\n{}\n```\n",
            super::diagram::LANGUAGE,
            serde_json::to_string(&projection)?
        ));
        for (predicate, value) in &shape.facts {
            if matches!(
                predicate.as_str().strip_prefix(SH),
                Some("targetClass" | "property")
            ) {
                continue;
            }
            output.push_str(&format!(
                "\n- {}: {}",
                constraint(predicate),
                value_text(value, prefixes, documented, &reference)
            ));
        }
        if shape.properties.is_empty() {
            output.push_str(
                "\n\nNo direct property shapes are declared. Consult the full \
                 shape for other constraints.\n",
            );
            continue;
        }
        output.push_str(
            "\n\n| Property path | Declared constraints | Property shape |\n| \
             --- | --- | --- |\n",
        );
        for (index, property) in shape.properties.iter().enumerate() {
            let simple = match property.paths.as_slice() {
                [Term::NamedNode(path)] if !anonymous(path) => Some(path),
                _ => None,
            };
            let path = match simple {
                Some(path) => resource(path, prefixes, documented),
                None if property.paths.is_empty() => {
                    "Missing path — consult full shape".into()
                },
                None if property.paths.len() > 1 => {
                    "Multiple path declarations — consult full shape".into()
                },
                None => "Complex path — consult full shape".into(),
            };
            let constraints = property
                .facts
                .iter()
                .map(|(predicate, value)| {
                    format!(
                        "{}: {}",
                        constraint(predicate),
                        value_text(value, prefixes, documented, &reference)
                    )
                })
                .collect::<Vec<_>>()
                .join("; "); // bounded: sixty-four facts.
            let mut constraints = if constraints.is_empty() {
                "No direct constraints declared".into()
            } else {
                constraints
            };
            if conflicting_counts(property) {
                constraints.push_str(
                    "; **Declared counts conflict — consult full shape**",
                );
            }
            if simple.is_none() {
                constraints.push_str(
                    "; constraints apply to the full path, not a direct \
                     property",
                );
            }
            let identity = if anonymous(&property.id) {
                format!(
                    "Anonymous property shape {} in {reference}",
                    index + 1
                )
            } else {
                resource(&property.id, prefixes, documented)
            };
            output.push_str(&format!(
                "| {path} | {constraints} | {identity} |\n"
            ));
        }
    }
    ensure!(
        output.len() <= 128 * 1024,
        "projected SHACL class views exceed 128 KiB"
    );
    body.push_str(&output);
    Ok(())
}

fn anonymous(id: &NamedNode) -> bool {
    id.as_str().starts_with("urn:grapharch:skolem:")
}
fn constraint(id: &NamedNode) -> String {
    format!(
        "sh:{}",
        escape(id.as_str().strip_prefix(SH).unwrap_or(id.as_str()))
    )
}
fn resource(
    id: &NamedNode,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
) -> String {
    let label = prefixes
        .iter()
        .filter(|(namespace, _)| id.as_str().starts_with(namespace.as_str()))
        .max_by_key(|(namespace, _)| namespace.len())
        .map(|(namespace, prefix)| {
            format!("{prefix}:{}", &id.as_str()[namespace.len()..])
        })
        .unwrap_or_else(|| id.as_str().to_owned());
    if !anonymous(id) &&
        (documented.contains(id) ||
            id.as_str().starts_with("https://") ||
            id.as_str().starts_with("http://"))
    {
        format!("[{}](<{}>)", escape(&label), id.as_str())
    } else {
        escape(&label)
    }
}
fn value_text(
    value: &Term,
    prefixes: &BTreeMap<String, String>,
    documented: &HashSet<NamedNode>,
    shape: &str,
) -> String {
    match value {
        Term::NamedNode(id) if anonymous(id) => {
            format!("Structured constraint — see {shape}")
        },
        Term::NamedNode(id) => resource(id, prefixes, documented),
        Term::Literal(value) => escape(value.value()),
        _ => format!("Structured constraint — see {shape}"),
    }
}
fn conflicting_counts(property: &Property) -> bool {
    let counts = |name: &'static str| {
        property.facts.iter().filter_map(move |(predicate, value)| {
            if predicate.as_str().strip_prefix(SH) != Some(name) {
                return None;
            }
            let Term::Literal(value) = value else {
                return None;
            };
            if value.datatype().as_str() !=
                "http://www.w3.org/2001/XMLSchema#integer"
            {
                return None;
            }
            value.value().parse::<u64>().ok()
        })
    };
    match (counts("minCount").max(), counts("maxCount").min()) {
        (Some(minimum), Some(maximum)) => minimum > maximum,
        _ => false,
    }
}
