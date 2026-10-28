//! Source-owned vocabulary-reference arrangement, resolved before rendering.
use super::*;

pub(super) struct Layout {
    pub overview: String,
    sections:     Vec<Section>,
}
pub(super) struct Section {
    pub kind:  String,
    pub title: String,
    order:     u32,
}
impl Layout {
    pub async fn read(
        view: &RevisionView,
        ontology: &NamedNode,
    ) -> Result<Self> {
        let titles =
            english(view, ontology, term("overviewTitle").as_str()).await?;
        ensure!(
            titles.len() <= 1,
            "ontology requires at most one English overviewTitle"
        );
        let overview = match titles.first() {
            None => "Overview".to_owned(),
            Some(value) => {
                title(
                    Literal::new_simple_literal(value).into(),
                    "overviewTitle",
                )?
            },
        };
        let mut sections = [
            ("classes", "Classes", 10),
            ("properties", "Properties", 20),
            ("individuals", "Named Individuals", 30),
            ("shapes", "Shapes", 40),
            ("schemes", "Concept Schemes", 41),
            ("concepts", "Concepts", 42),
            ("collections", "Collections", 43),
            ("other", "Other Terms", 50),
        ]
        .into_iter()
        .map(|(kind, name, order)| {
            Section { kind: kind.into(), title: name.into(), order }
        })
        .collect::<Vec<_>>();
        let mut rows = view.objects(ontology, "referenceSection").await?;
        let mut seen = HashSet::new();
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(node) = &row["value"] else {
                anyhow::bail!(
                    "referenceSection must identify a section resource"
                )
            };
            ensure!(
                seen.len() < 8,
                "at most eight reference section overrides are supported"
            );
            let kind = title(
                view.optional(node, "sectionKind").await?.ok_or_else(|| {
                    anyhow::anyhow!("reference section requires sectionKind")
                })?,
                "sectionKind",
            )?;
            ensure!(
                seen.insert(kind.clone()),
                "duplicate reference section kind {kind}"
            );
            let section =
                sections.iter_mut().find(|s| s.kind == kind).ok_or_else(
                    || anyhow::anyhow!("unknown reference section kind {kind}"),
                )?;
            let titles =
                english(view, node, "http://purl.org/dc/terms/title").await?;
            ensure!(
                titles.len() == 1,
                "reference section requires one English title"
            );
            section.title = title(
                Literal::new_simple_literal(&titles[0]).into(),
                "reference section title",
            )?;
            let Term::Literal(order) =
                view.optional(node, "order").await?.ok_or_else(|| {
                    anyhow::anyhow!("reference section requires order")
                })?
            else {
                anyhow::bail!("reference section order must be an integer")
            };
            ensure!(
                order.datatype().as_str() ==
                    "http://www.w3.org/2001/XMLSchema#integer",
                "reference section order must have datatype xsd:integer"
            );
            section.order = order.value().parse::<u32>()?;
            ensure!(
                section.order <= 1000,
                "reference section order exceeds 1000"
            );
        }
        sections.sort_by_key(|section| section.order);
        ensure!(
            sections
                .windows(2)
                .all(|pair| pair[0].order != pair[1].order),
            "reference sections require distinct orders"
        );
        Ok(Self { overview, sections })
    }

    pub async fn group(
        self,
        dataset: &Dataset,
        view: &RevisionView,
        subjects: &[NamedNode],
    ) -> Result<Vec<(Section, Vec<NamedNode>)>> {
        let pattern = view.pattern("?subject a ?kind", 0);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?subject ?kind WHERE {{ {pattern} \
                 FILTER(isIRI(?subject) && isIRI(?kind)) }} LIMIT {}",
                crate::publication::limits::SOURCE_TYPE_ASSERTIONS + 1
            ))
            .await?;
        let mut types: BTreeMap<NamedNode, HashSet<String>> = BTreeMap::new();
        let mut count = 0;
        while let Some(row) = rows.next().await? {
            count += 1;
            ensure!(
                count <= crate::publication::limits::SOURCE_TYPE_ASSERTIONS,
                "ontology reference exceeds type assertion budget"
            );
            if let (Term::NamedNode(subject), Term::NamedNode(kind)) =
                (&row["subject"], &row["kind"])
            {
                types
                    .entry(subject.clone())
                    .or_default()
                    .insert(kind.as_str().to_owned());
            }
        }
        let mut groups: BTreeMap<&str, Vec<NamedNode>> = BTreeMap::new();
        for subject in subjects {
            let kinds = types.get(subject).ok_or_else(|| {
                anyhow::anyhow!(
                    "documented term has no declared RDF type: {subject}"
                )
            })?;
            let has = |iri: &str| kinds.contains(iri);
            // Punned individuals remain individuals and expose all their other
            // types in their reference entry; no duplicate anchors or terms.
            let kind = if has(
                "http://www.w3.org/2004/02/skos/core#ConceptScheme",
            ) {
                "schemes"
            } else if has("http://www.w3.org/2004/02/skos/core#Concept") {
                "concepts"
            } else if has("http://www.w3.org/2004/02/skos/core#Collection") ||
                has("http://www.w3.org/2004/02/skos/core#OrderedCollection")
            {
                "collections"
            } else if has("http://www.w3.org/2002/07/owl#NamedIndividual") {
                "individuals"
            } else if has("http://www.w3.org/2002/07/owl#Class") ||
                has("http://www.w3.org/2000/01/rdf-schema#Class") ||
                has("http://www.w3.org/2000/01/rdf-schema#Datatype")
            {
                "classes"
            } else if [
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#Property",
                "http://www.w3.org/2002/07/owl#ObjectProperty",
                "http://www.w3.org/2002/07/owl#DatatypeProperty",
                "http://www.w3.org/2002/07/owl#AnnotationProperty",
            ]
            .iter()
            .any(|iri| has(iri))
            {
                "properties"
            } else if has("http://www.w3.org/ns/shacl#NodeShape") ||
                has("http://www.w3.org/ns/shacl#PropertyShape")
            {
                "shapes"
            } else {
                "other"
            };
            groups.entry(kind).or_default().push(subject.clone());
        }
        Ok(self
            .sections
            .into_iter()
            .filter_map(|section| {
                groups
                    .remove(section.kind.as_str())
                    .map(|terms| (section, terms))
            })
            .collect())
    }
}
fn title(value: Term, field: &str) -> Result<String> {
    let Term::Literal(value) = value else {
        anyhow::bail!("{field} must be text")
    };
    let text = value.value();
    ensure!(
        !text.trim().is_empty() &&
            text.len() <= 160 &&
            !text.contains([
                '\n', '\r', '|', '[', ']', '*', '#', '<', '>', '`', '\\'
            ]),
        "{field} must be a nonempty plain-text heading of at most 160 bytes"
    );
    Ok(text.into())
}
