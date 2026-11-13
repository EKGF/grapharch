//! The trust boundary of a revision build: only the controlling definitions
//! may define publications. A content contribution that asserts publication
//! definition vocabulary, or describes a resource the definitions declare, is
//! rejected rather than merged.
use {
    super::DEFINITIONS_PROCESSOR,
    crate::publication::{
        RDF_TYPE,
        dataset::{CONTROL, Dataset},
        term,
        view::RevisionView,
    },
    anyhow::{Result, bail},
    oxrdf::NamedNode,
};

/// Classes whose instances are publication definitions.
const CLASSES: [&str; 15] = [
    "Project",
    "Target",
    "Profile",
    "Placement",
    "Scope",
    "CollectorUse",
    "DocumentorUse",
    "ReferenceSection",
    "MetadataRow",
    "Navigation",
    "NavigationEntry",
    "Theme",
    "CoverBlock",
    "Notice",
    "CopyrightHolder",
];

/// Defined resources that no content contribution may describe.
const DECLARED: [&str; 13] = [
    "Project",
    "Target",
    "Profile",
    "Placement",
    "Scope",
    "ReferenceSection",
    "MetadataRow",
    "Navigation",
    "NavigationEntry",
    "Theme",
    "CoverBlock",
    "Notice",
    "CopyrightHolder",
];

/// Predicates that only publication definitions assert. Collector output such
/// as fragments, topics, references and assets uses none of them.
const PREDICATES: [&str; 55] = [
    "target",
    "scope",
    "profile",
    "renderer",
    "destination",
    "placement",
    "parentPlacement",
    "component",
    "collector",
    "documentor",
    "collectorType",
    "documentorType",
    "include",
    "exclude",
    "compose",
    "traverse",
    "maxDepth",
    "notice",
    "copyrightHolder",
    "metadataSource",
    "derivedMetadata",
    "includedIn",
    "referenceSection",
    "termType",
    "definedTermsOnly",
    "sectionKind",
    "sectionRole",
    "requiredSectionRole",
    "requiredMetadata",
    "allowedPublicationStatus",
    "theme",
    "baseTheme",
    "coverBlock",
    "requiredMetadataField",
    "familyPlacement",
    "layout",
    "navigation",
    "navigationEntry",
    "metadataRow",
    "publisher",
    "publicationStatus",
    "publicationDate",
    "thisVersion",
    "latestVersion",
    "previousVersion",
    "editorDraft",
    "editor",
    "author",
    "formerEditor",
    "license",
    "copyright",
    "subtitle",
    "containsTarget",
    "documentationGraphPath",
    "contentSource",
];

/// Engine-owned catalogue retained alongside source contributions.
const CATALOGUE: &str = "urn:grapharch:processor:definition-catalogue";

/// Reject the first collected contribution, other than the controlling
/// definitions, that defines or redefines a publication.
pub(super) async fn reject_content_definitions(
    dataset: &Dataset,
    definitions: &RevisionView,
    snapshot: &NamedNode,
) -> Result<()> {
    let classes = values(&CLASSES);
    let predicates = values(&PREDICATES);
    // Collector output describes its own collector use as a ga:Processor, so
    // configured uses are guarded by the definitions-only dispatch instead.
    let declared = format!(
        "{} {} <{}>",
        values(&DECLARED),
        term("Component"),
        crate::publication::governance::AUTHORITY
    );
    for (bindings, pattern, what) in [
        (
            format!("VALUES ?predicate {{ {predicates} }}"),
            "?subject ?predicate ?object".to_owned(),
            "publication definition predicate",
        ),
        (
            format!("VALUES ?object {{ {classes} }}"),
            format!(
                "?subject <{RDF_TYPE}> ?object BIND(<{RDF_TYPE}> AS \
                 ?predicate)"
            ),
            "publication definition class",
        ),
        (
            format!(
                "VALUES ?class {{ {declared} }} {}",
                definitions
                    .pattern(&format!("?subject <{RDF_TYPE}> ?class"), 1)
            ),
            "?subject ?predicate ?object".to_owned(),
            "assertion about a defined publication resource",
        ),
    ] {
        let mut rows = dataset
            .select(format!(
                "SELECT ?processor ?subject ?predicate WHERE {{ {bindings} \
                 GRAPH {snapshot} {{ ?_owner {head} ?_version }} GRAPH \
                 <{CONTROL}> {{ ?_version {data} ?_attempt . ?_physical \
                 {attempt} ?_attempt . ?_attempt {stage} \"collect\" ; \
                 {collector} ?processor . FILTER(?processor NOT IN (<{}>, \
                 <{CATALOGUE}>)) }} GRAPH ?_physical {{ {pattern} }} }} LIMIT \
                 1",
                DEFINITIONS_PROCESSOR,
                head = term("head"),
                data = term("data"),
                attempt = term("attempt"),
                stage = term("stage"),
                collector = term("collector"),
            ))
            .await?;
        if let Some(row) = rows.next().await? {
            bail!(
                "content contribution {} asserts {what} {} on {}; only the \
                 controlling definitions may define publications",
                row["processor"],
                row["predicate"],
                row["subject"]
            )
        }
    }
    Ok(())
}

fn values(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| term(name).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use {
        super::{CLASSES, DECLARED, PREDICATES},
        oxigraph::{io::RdfFormat, sparql::QueryResults, store::Store},
    };

    #[test]
    fn every_guarded_term_is_declared_publication_vocabulary()
    -> anyhow::Result<()> {
        let store = Store::new()?;
        store.load_from_reader(
            RdfFormat::Turtle,
            include_bytes!("../../../docs/other/ontology/publication.ttl")
                .as_slice(),
        )?;
        for name in CLASSES
            .iter()
            .chain(PREDICATES.iter())
            .chain(DECLARED.iter())
        {
            let QueryResults::Boolean(declared) = oxigraph::sparql::SparqlEvaluator::new()
                .parse_query(&format!(
                    "ASK {{ <https://ekgf.org/ontology/grapharch#{name}> a ?kind . VALUES ?kind {{ <http://www.w3.org/2002/07/owl#Class> <http://www.w3.org/2002/07/owl#ObjectProperty> <http://www.w3.org/2002/07/owl#DatatypeProperty> }} }}"
                ))?
                .on_store(&store)
                .execute()?
            else {
                anyhow::bail!("expected ASK result")
            };
            assert!(declared, "ga:{name} is not declared");
        }
        Ok(())
    }
}
