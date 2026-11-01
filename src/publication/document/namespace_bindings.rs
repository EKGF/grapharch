//! Display namespace bindings retained as source RDF, independent of parser
//! prefix spellings. These labels never change a resource's canonical IRI.
use {
    crate::publication::{dataset::Dataset, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeMap,
};

pub(crate) async fn read(
    dataset: &Dataset,
    view: &RevisionView,
    mut defaults: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let pattern = view.pattern(
        "?ontology a <http://www.w3.org/2002/07/owl#Ontology> ; \
         <http://www.w3.org/ns/shacl#declare> ?declaration . \
         ?declaration <http://www.w3.org/ns/shacl#prefix> ?prefix ; \
         <http://www.w3.org/ns/shacl#namespace> ?namespace",
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?namespace ?prefix WHERE {{ {pattern} }} ORDER \
             BY ?namespace ?prefix LIMIT 257"
        ))
        .await?;
    // bounded: at most 256 explicit bindings per publication, never source
    // data.
    let mut declared = BTreeMap::new();
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        count += 1;
        ensure!(
            count <= 256,
            "source namespace declaration budget exceeded"
        );
        let (Term::Literal(namespace), Term::Literal(prefix)) =
            (&row["namespace"], &row["prefix"])
        else {
            anyhow::bail!("sh:declare namespace and prefix must be literals")
        };
        NamedNode::new(namespace.value())?;
        ensure!(
            !prefix.value().is_empty(),
            "source namespace prefix cannot be empty"
        );
        // Several local aliases for one namespace are legal. Choose the first
        // lexical alias deterministically; an authored binding outranks
        // defaults.
        declared
            .entry(namespace.value().to_owned())
            .or_insert_with(|| prefix.value().to_owned());
    }
    defaults.extend(declared);
    Ok(defaults)
}
