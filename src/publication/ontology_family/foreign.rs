//! Subjects outside an ontology's namespace that its own document describes,
//! such as the classes a mapping ontology relates or a datatype it declares.
use {
    super::super::{
        dataset::Dataset,
        limits::FAMILY_RESOURCES,
        view::RevisionView,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{Literal, NamedNode, Term},
};

/// Declared OWL entities of one source document, outside `namespace`, with
/// at least one statement beyond their declaration.
pub(super) async fn subjects(
    dataset: &Dataset,
    source: &RevisionView,
    ontology: &NamedNode,
    namespace: &str,
) -> Result<Vec<NamedNode>> {
    let declared = source.pattern("?subject a ?kind", 0);
    let described = source.pattern("?subject ?predicate ?object", 1);
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?subject WHERE {{ VALUES ?kind {{ \
             <http://www.w3.org/2002/07/owl#Class> \
             <http://www.w3.org/2000/01/rdf-schema#Class> \
             <http://www.w3.org/2000/01/rdf-schema#Datatype> \
             <http://www.w3.org/2002/07/owl#ObjectProperty> \
             <http://www.w3.org/2002/07/owl#DatatypeProperty> \
             <http://www.w3.org/2002/07/owl#AnnotationProperty> \
             <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> \
             <http://www.w3.org/2002/07/owl#NamedIndividual> }} {declared} \
             FILTER(isIRI(?subject) && ?subject != {ontology} && \
             !STRSTARTS(STR(?subject), {}) && !STRSTARTS(STR(?subject), \
             \"urn:grapharch:skolem:\")) FILTER EXISTS {{ {described} \
             FILTER(?predicate != \
             <http://www.w3.org/1999/02/22-rdf-syntax-ns#type>) }} }} ORDER BY \
             ?subject LIMIT {}",
            Literal::new_simple_literal(namespace),
            FAMILY_RESOURCES + 1
        ))
        .await?;
    let mut subjects = Vec::new(); // bounded: FAMILY_RESOURCES.
    while let Some(row) = rows.next().await? {
        ensure!(
            subjects.len() < FAMILY_RESOURCES,
            "ontology document describes more than {FAMILY_RESOURCES} \
             subjects outside its namespace"
        );
        let Term::NamedNode(subject) = &row["subject"] else {
            bail!("described subjects require named identities")
        };
        subjects.push(subject.clone());
    }
    Ok(subjects)
}
