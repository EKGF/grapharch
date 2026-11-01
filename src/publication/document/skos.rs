//! Source-asserted scheme membership and concept navigation, shared by
//! reference publishers. This arranges fragments before rendering; it does not
//! infer RDF.
use {
    super::annotated_term::escape,
    crate::publication::{dataset::Dataset, limits, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Quad, Term},
    std::collections::{BTreeMap, BTreeSet, HashSet},
};
const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";

#[derive(Clone, Default)]
pub(crate) struct Decoration {
    pub depth:    Option<u32>,
    pub body:     String,
    pub warnings: Vec<(String, String)>,
}

impl Decoration {
    pub fn diagnostics(
        &self,
        context: &crate::publication::diagnostic_graph::Context,
    ) -> Result<Vec<Quad>> {
        let mut quads = Vec::new();
        for (code, message) in &self.warnings {
            let job = crate::publication::dataset::identity("skos-quality", &[
                context.about.as_str(),
                code,
                context.revision.as_str(),
            ]);
            let diagnostic = crate::publication::diagnostics::Diagnostic {
                severity:
                    crate::publication::diagnostics::Severity::Warning,
                code:            code.clone(),
                message:         message.clone(),
                reference:       context.about.as_str().into(),
                source_fragment: None,
            };
            quads.extend(
                crate::publication::diagnostic_graph::quads(
                    context,
                    job.as_str(),
                    &[diagnostic],
                )?
                .into_iter()
                .filter(|quad| {
                    quad.predicate !=
                        crate::publication::term("diagnosticCount")
                }),
            );
        }
        Ok(quads)
    }
}

pub(crate) struct SchemeArrangement {
    concepts:    BTreeSet<NamedNode>,
    schemes:     BTreeSet<NamedNode>,
    memberships: BTreeMap<NamedNode, BTreeSet<NamedNode>>,
    members:     BTreeMap<NamedNode, BTreeSet<NamedNode>>,
    canonical:   BTreeMap<NamedNode, Vec<NamedNode>>,
    children:    BTreeMap<NamedNode, BTreeSet<NamedNode>>,
    tops:        BTreeMap<NamedNode, BTreeSet<NamedNode>>,
    labels:      BTreeMap<NamedNode, (u8, String)>,
}
impl SchemeArrangement {
    pub async fn read(
        dataset: &Dataset,
        view: &RevisionView,
        selected: &HashSet<NamedNode>,
    ) -> Result<Self> {
        let mut result = Self {
            concepts:    BTreeSet::new(),
            schemes:     BTreeSet::new(),
            memberships: BTreeMap::new(),
            members:     BTreeMap::new(),
            canonical:   BTreeMap::new(),
            children:    BTreeMap::new(),
            tops:        BTreeMap::new(),
            labels:      BTreeMap::new(),
        };
        let pattern = view.pattern("?subject ?predicate ?object", 0);
        let mut rows = dataset.select(format!("SELECT DISTINCT ?subject ?predicate ?object WHERE {{ {pattern} VALUES ?predicate {{ <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <{SKOS}inScheme> <{SKOS}topConceptOf> <{SKOS}hasTopConcept> <{SKOS}broader> <{SKOS}narrower> <{SKOS}prefLabel> <http://www.w3.org/2000/01/rdf-schema#label> <http://purl.org/dc/terms/title> }} FILTER(isIRI(?subject)) }} ORDER BY ?subject ?predicate ?object LIMIT {}",limits::SOURCE_TYPE_ASSERTIONS+1)).await?;
        let mut count = 0;
        // bounded: this index shares the publication's source-assertion and
        // selected-resource limits; source RDF is streamed, never hydrated.
        let mut relations = Vec::new();
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= limits::SOURCE_TYPE_ASSERTIONS,
                "SKOS reference exceeds source assertion budget"
            );
            let (Term::NamedNode(subject), Term::NamedNode(predicate)) =
                (&row["subject"], &row["predicate"])
            else {
                unreachable!()
            };
            if !selected.contains(subject) {
                continue;
            }
            match (&row["object"], predicate.as_str()) {
                (
                    Term::NamedNode(kind),
                    "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                ) => {
                    if kind.as_str() == format!("{SKOS}Concept") {
                        result.concepts.insert(subject.clone());
                    }
                    if kind.as_str() == format!("{SKOS}ConceptScheme") {
                        result.schemes.insert(subject.clone());
                    }
                },
                (Term::Literal(value), property)
                    if matches!(
                        property,
                        "http://www.w3.org/2004/02/skos/core#prefLabel" |
                            "http://www.w3.org/2000/01/rdf-schema#label" |
                            "http://purl.org/dc/terms/title"
                    ) && super::description::selected(value, "en") =>
                {
                    let rank = if property.ends_with("prefLabel") {
                        0
                    } else if property.ends_with("label") {
                        1
                    } else {
                        2
                    };
                    let candidate = (rank, value.value().to_owned());
                    if result
                        .labels
                        .get(subject)
                        .is_none_or(|old| &candidate < old)
                    {
                        result.labels.insert(subject.clone(), candidate);
                    }
                },
                (Term::NamedNode(object), _) => {
                    relations.push((
                        subject.clone(),
                        predicate.clone(),
                        object.clone(),
                    ))
                },
                _ => {},
            }
        }
        for (subject, predicate, object) in relations {
            let property = predicate.as_str().strip_prefix(SKOS).unwrap_or("");
            let (concept, scheme) = if property == "hasTopConcept" {
                (&object, &subject)
            } else {
                (&subject, &object)
            };
            if matches!(
                property,
                "inScheme" | "topConceptOf" | "hasTopConcept"
            ) && result.concepts.contains(concept) &&
                result.schemes.contains(scheme)
            {
                result
                    .memberships
                    .entry(concept.clone())
                    .or_default()
                    .insert(scheme.clone());
                if property != "inScheme" {
                    result
                        .tops
                        .entry(scheme.clone())
                        .or_default()
                        .insert(concept.clone());
                }
            }
            let (parent, child) = if property == "broader" {
                (&object, &subject)
            } else {
                (&subject, &object)
            };
            if matches!(property, "broader" | "narrower") &&
                result.concepts.contains(parent) &&
                result.concepts.contains(child)
            {
                result
                    .children
                    .entry(parent.clone())
                    .or_default()
                    .insert(child.clone());
            }
        }
        for (concept, schemes) in &result.memberships {
            for scheme in schemes {
                result
                    .members
                    .entry(scheme.clone())
                    .or_default()
                    .insert(concept.clone());
            }
            if let Some(scheme) = schemes.first() {
                result
                    .canonical
                    .entry(scheme.clone())
                    .or_default()
                    .push(concept.clone());
            }
        }
        Ok(result)
    }

    pub fn is_concept(&self, subject: &NamedNode) -> bool {
        self.concepts.contains(subject)
    }

    pub fn is_scheme(&self, subject: &NamedNode) -> bool {
        self.schemes.contains(subject)
    }

    pub fn unassigned(&self) -> Vec<NamedNode> {
        self.concepts
            .iter()
            .filter(|subject| !self.memberships.contains_key(*subject))
            .cloned()
            .collect()
    }

    pub fn canonical_scheme(&self, concept: &NamedNode) -> Option<&NamedNode> {
        self.memberships.get(concept).and_then(BTreeSet::first)
    }

    pub fn canonical_members(&self, scheme: &NamedNode) -> Vec<NamedNode> {
        self.canonical.get(scheme).cloned().unwrap_or_default()
    }

    fn link(&self, subject: &NamedNode) -> String {
        let label = self
            .labels
            .get(subject)
            .map(|(_, label)| label.as_str())
            .unwrap_or(subject.as_str());
        format!("[{}]({})", escape(label), subject.as_str())
    }

    pub fn decoration(&self, subject: &NamedNode, depth: u32) -> Decoration {
        let mut decoration = Decoration::default();
        if let Some(schemes) = self.memberships.get(subject) {
            decoration.depth = Some(depth + 1);
            if schemes.len() > 1 {
                decoration.body = format!(
                    "\n\n**Also listed in:** {}. The complete reference entry \
                     appears once.\n",
                    schemes
                        .iter()
                        .skip(1)
                        .map(|scheme| self.link(scheme))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        if !self.is_scheme(subject) {
            return decoration;
        }
        let Some(members) = self.members.get(subject) else {
            return decoration;
        };
        let children = members
            .iter()
            .filter_map(|parent| self.children.get(parent))
            .flat_map(|children| children.iter())
            .filter(|child| members.contains(*child))
            .collect::<HashSet<_>>();
        let mut roots = self.tops.get(subject).cloned().unwrap_or_default();
        roots.extend(
            members
                .iter()
                .filter(|member| !children.contains(*member))
                .cloned(),
        );
        let mut visited = HashSet::new();
        let mut active = HashSet::new();
        let mut cycle = false;
        let mut truncated = false;
        decoration
            .body
            .push_str("\n\n**Concepts in this scheme**\n\n");
        // bounded: each selected concept is expanded once; repeated parents and
        // cycles retain links without recursively duplicating the graph.
        for root in roots.into_iter().chain(members.iter().cloned()) {
            if visited.contains(&root) {
                continue;
            }
            let mut stack = vec![(root, 0usize, false)];
            while let Some((concept, level, exit)) = stack.pop() {
                if exit {
                    active.remove(&concept);
                    continue;
                }
                let repeat = visited.contains(&concept);
                cycle |= active.contains(&concept);
                let other = self.memberships[&concept].first() != Some(subject);
                decoration.body.push_str(&format!(
                    "{}- {}{}\n",
                    "  ".repeat(level),
                    self.link(&concept),
                    if other {
                        " — reference entry in another scheme"
                    } else if repeat {
                        " — see reference entry"
                    } else {
                        ""
                    }
                ));
                if repeat {
                    continue;
                }
                visited.insert(concept.clone());
                active.insert(concept.clone());
                stack.push((concept.clone(), level, true));
                if let Some(children) = self.children.get(&concept) {
                    if level == 7 &&
                        children.iter().any(|child| members.contains(child))
                    {
                        truncated = true;
                        continue;
                    }
                    for child in children
                        .iter()
                        .rev()
                        .filter(|child| members.contains(*child))
                    {
                        stack.push((child.clone(), level + 1, false));
                    }
                }
            }
        }
        for (condition, code, message) in [
            (
                cycle,
                "skos-hierarchy-cycle",
                "An asserted broader/narrower cycle exists in this concept \
                 scheme. Repeated concepts are linked without inventing a \
                 tree.",
            ),
            (
                truncated,
                "skos-hierarchy-depth",
                "The asserted concept hierarchy exceeds eight display levels. \
                 Remaining concepts are listed separately; source \
                 relationships remain in their reference entries.",
            ),
        ] {
            if condition {
                decoration
                    .body
                    .push_str(&format!("\n> [!WARNING]\n> {message}\n"));
                decoration.warnings.push((code.into(), message.into()));
            }
        }
        decoration
    }
}
