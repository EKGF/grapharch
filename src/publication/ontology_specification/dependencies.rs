//! Explicit reused-term selection from already collected, pinned RDF sources.
use {
    super::{Dataset, RevisionView, q, term},
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Quad, Term},
    std::collections::{BTreeMap, BTreeSet, HashSet},
};

pub(super) async fn selected(
    dataset: &Dataset,
    view: &RevisionView,
    scope: &NamedNode,
    owned: &HashSet<NamedNode>,
) -> Result<Vec<NamedNode>> {
    let mut members = view.objects(scope, "include").await?;
    let mut selected = BTreeSet::new(); // bounded: coordinated resource and identity-byte budgets.
    let mut identity_bytes: usize =
        owned.iter().map(|id| id.as_str().len()).sum();
    while let Some(row) = members.next().await? {
        let Term::NamedNode(subject) = &row["value"] else {
            anyhow::bail!("selected ontology term must be an IRI")
        };
        if !owned.contains(subject) && selected.insert(subject.clone()) {
            identity_bytes += subject.as_str().len();
        }
        ensure!(
            selected.len() + owned.len() <=
                crate::publication::limits::FAMILY_RESOURCES &&
                identity_bytes <=
                    crate::publication::limits::REFERENCE_INDEX_BYTES,
            "ontology specification exceeds coordinated resource or \
             identity-byte budget"
        );
    }
    for subject in &selected {
        let pattern = view.pattern(&format!("{subject} a ?kind"), 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?kind WHERE {{ {pattern} }} LIMIT 65"
            ))
            .await?;
        let mut categories = BTreeSet::new();
        let mut property = false;
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= 64,
                "selected term exceeds type budget: {subject}"
            );
            let Term::NamedNode(kind) = &row["kind"] else {
                continue
            };
            match kind.as_str() {
                "http://www.w3.org/2002/07/owl#Class" |
                "http://www.w3.org/2000/01/rdf-schema#Class" => {
                    categories.insert("class");
                },
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#Property" => {
                    property = true
                },
                "http://www.w3.org/2002/07/owl#ObjectProperty" => {
                    categories.insert("object property");
                },
                "http://www.w3.org/2002/07/owl#DatatypeProperty" => {
                    categories.insert("datatype property");
                },
                "http://www.w3.org/2002/07/owl#AnnotationProperty" => {
                    categories.insert("annotation property");
                },
                _ => (),
            }
        }
        ensure!(
            !categories.is_empty() || property,
            "selected term {subject} is missing or has no supported \
             class/property type in explicit sources"
        );
        ensure!(
            categories.len() <= 1,
            "selected term {subject} has ambiguous incompatible \
             class/property types"
        );
        ensure!(
            !(property && categories.contains("class")),
            "selected term {subject} is both class and property"
        );
    }
    Ok(selected.into_iter().collect())
}

pub(super) async fn sources(
    dataset: &Dataset,
    view: &RevisionView,
    subjects: &[NamedNode],
) -> Result<BTreeMap<NamedNode, BTreeSet<String>>> {
    if subjects.is_empty() {
        return Ok(BTreeMap::new())
    }
    // bounded: coordinated resource/identity-byte budgets, and the query cap
    // below.
    let values = subjects
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    ensure!(
        values.len() <= 32 * 1024,
        "selected term identities exceed query budget"
    );
    let pattern = view.pattern("?subject ?definitionPredicate ?definition", 0);
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?subject ?source WHERE {{ VALUES ?subject {{ \
             {values} }} VALUES ?definitionPredicate {{ \
             <http://purl.org/dc/terms/description> \
             <http://www.w3.org/2000/01/rdf-schema#comment> \
             <http://www.w3.org/2004/02/skos/core#definition> \
             <http://www.w3.org/ns/prov#definition> }} \
             {pattern} FILTER(isLiteral(?definition) && \
             (LANG(?definition) = \"\" || LCASE(LANG(?definition)) = \"en\")) GRAPH <{}> {{ ?_attempt0 {} ?source }} }} \
             ORDER BY ?subject ?source",
            crate::publication::dataset::CONTROL,
            term("source")
        ))
        .await?;
    let mut result = BTreeMap::<NamedNode, BTreeSet<String>>::new(); // bounded: 200 selected terms, 64 source identities each.
    let mut bytes = 0;
    while let Some(row) = rows.next().await? {
        let (Term::NamedNode(subject), Term::Literal(source)) =
            (&row["subject"], &row["source"])
        else {
            anyhow::bail!("invalid selected term source provenance")
        };
        let entries = result.entry(subject.clone()).or_default();
        bytes += source.value().len();
        ensure!(
            bytes <= 1024 * 1024,
            "selected term provenance exceeds byte budget"
        );
        entries.insert(source.value().to_owned());
        ensure!(
            entries.len() <= 64,
            "selected term exceeds source provenance budget"
        );
    }
    Ok(result)
}

pub(super) fn attribution(
    target: &NamedNode,
    component: &NamedNode,
    contribution: &NamedNode,
    subject: &NamedNode,
    sources: &BTreeSet<String>,
    order: u32,
) -> Vec<Quad> {
    let fragment =
        crate::publication::dataset::identity("reused-term-attribution", &[
            target.as_str(),
            subject.as_str(),
        ]);
    let namespace = subject
        .as_str()
        .rfind(['#', '/'])
        .map(|at| &subject.as_str()[..=at])
        .unwrap_or(subject.as_str());
    let mut body = format!(
        "**Source namespace:** `{namespace}`\n\n**Definition sources:**\n"
    );
    for source in sources {
        match url::Url::parse(source) {
            Ok(url) if matches!(url.scheme(), "http" | "https") => {
                let link = url.as_str().replace('<', "%3C").replace('>', "%3E");
                body.push_str(&format!("\n- <{link}>"));
            },
            _ => {
                let escaped = source.replace('`', "\\`");
                body.push_str(&format!("\n- `{escaped}`"));
            },
        }
    }
    vec![
        q(&fragment, "contribution", contribution.clone()),
        q(component, "fragment", fragment.clone()),
        q(&fragment, "type", term("Fragment")),
        q(&fragment, "subject", subject.clone()),
        q(
            &fragment,
            "body",
            Literal::new_simple_literal(body),
        ),
        q(
            &fragment,
            "format",
            Literal::new_simple_literal("text/markdown"),
        ),
        q(&fragment, "order", Literal::from(order)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_definition_sources_are_clickable_and_local_sources_stay_literal() {
        let id = NamedNode::new("urn:test").unwrap();
        let sources = BTreeSet::from([
            "https://example.org/vocab.rdf?version=1&language=en".into(),
            "/tmp/definition.ttl".into(),
        ]);
        let quads = attribution(&id, &id, &id, &id, &sources, 0);
        let body = quads
            .iter()
            .find(|quad| quad.predicate == term("body"))
            .unwrap();
        let Term::Literal(body) = &body.object else {
            panic!("body literal required")
        };
        let mut html = String::new();
        pulldown_cmark::html::push_html(
            &mut html,
            pulldown_cmark::Parser::new(body.value()),
        );
        assert!(html.contains(
            "href=\"https://example.org/vocab.rdf?version=1&amp;language=en\""
        ));
        assert!(html.contains("<code>/tmp/definition.ttl</code>"));
        assert!(!html.contains("href=\"/tmp"));
    }
}
