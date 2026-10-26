//! Small RDF hint adapter for explicit ontology modules. Native assertions stay
//! untouched; only explicitly typed modules get normalized membership links.
use {
    crate::publication::{RDF_TYPE, term},
    anyhow::{Result, ensure},
    oxrdf::{GraphName, NamedNode, NamedOrBlankNode, Quad, Term},
    std::collections::{BTreeSet, HashSet},
};
#[derive(Default)]
pub(super) struct ModuleHints {
    modules: BTreeSet<NamedNode>,
    parts:   HashSet<(NamedNode, Term)>,
}
impl ModuleHints {
    pub fn observe(&mut self, quad: &Quad) -> Result<()> {
        let NamedOrBlankNode::NamedNode(subject) = &quad.subject else {
            return Ok(())
        };
        if quad.predicate.as_str() == RDF_TYPE &&
            matches!(&quad.object, Term::NamedNode(kind) if kind.as_str() == "https://spec.edmcouncil.org/fibo/ontology/FND/Utilities/AnnotationVocabulary/Module" || kind == &term("OntologyModule"))
        {
            ensure!(
                self.modules.len() < 4096,
                "RDF source exceeds module hint budget"
            );
            self.modules.insert(subject.clone());
        }
        if quad.predicate.as_str() == "http://purl.org/dc/terms/hasPart" {
            ensure!(
                self.parts.len() < 65536,
                "RDF source exceeds module membership hint budget"
            );
            self.parts.insert((subject.clone(), quad.object.clone()));
        }
        Ok(())
    }

    pub fn finish(self) -> Result<Vec<Quad>> {
        let mut quads = Vec::new();
        for subject in &self.modules {
            quads.push(Quad::new(
                subject.clone(),
                NamedNode::new_unchecked(RDF_TYPE),
                term("OntologyModule"),
                GraphName::DefaultGraph,
            ));
        }
        for (owner, member) in self.parts {
            if self.modules.contains(&owner) {
                ensure!(
                    matches!(member, Term::NamedNode(_)),
                    "ontology module membership must identify a named resource"
                );
                quads.push(Quad::new(
                    owner,
                    term("moduleMember"),
                    member,
                    GraphName::DefaultGraph,
                ));
            }
        }
        Ok(quads)
    }
}
