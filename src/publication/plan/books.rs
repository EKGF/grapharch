//! Book boundaries are resolved into the KG plan, never guessed by renderers.
use {super::*, std::collections::BTreeMap};
struct Part {
    placement: NamedNode,
    subject:   NamedNode,
    parent:    Option<NamedNode>,
    module:    bool,
    ontology:  bool,
    boundary:  Option<bool>,
    title:     String,
}
pub(super) async fn append(
    selection: &Selection<'_>,
    placements: &[(NamedNode, NamedNode, NamedNode)],
    dataset: &Dataset,
    stream: &mut ContributionStream,
) -> Result<()> {
    let mut parts = Vec::new();
    let mut index = HashMap::new();
    for (placement, component, fragment) in placements {
        ensure!(
            parts.len() < super::super::limits::SPECIFICATION_PAGES,
            "bookshelf exceeds component budget"
        );
        let subject = selection.view.node(fragment, "subject").await?;
        let module =
            selection.view.has_type(&subject, "OntologyModule").await?;
        let types = selection.view.pattern(
            &format!("{subject} a ?bookRootKind . VALUES ?bookRootKind {{ <http://www.w3.org/2002/07/owl#Ontology> <http://www.w3.org/2004/02/skos/core#ConceptScheme> }}"),
            0,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT ?_owner0 WHERE {{ {types} }} LIMIT 1"
            ))
            .await?;
        let ontology = rows.next().await?.is_some();
        let parent = match selection
            .view
            .optional(placement, "parentPlacement")
            .await?
        {
            None => None,
            Some(Term::NamedNode(parent)) => Some(parent),
            _ => bail!("parent placement must be an IRI"),
        };
        if let Some(parent) = &parent {
            ensure!(
                index.contains_key(parent),
                "book parent must precede child"
            );
        }
        let boundary =
            match selection.view.optional(&subject, "bookBoundary").await? {
                None => None,
                Some(Term::Literal(v))
                    if v.datatype().as_str() ==
                        "http://www.w3.org/2001/XMLSchema#boolean" =>
                {
                    Some(match v.value() {
                        "true" | "1" => true,
                        "false" | "0" => false,
                        _ => bail!("invalid bookBoundary boolean"),
                    })
                },
                _ => bail!("bookBoundary must be an xsd:boolean"),
            };
        ensure!(
            boundary.is_none() || module,
            "bookBoundary applies only to modules"
        );
        let title = match selection
            .view
            .optional_by(
                component,
                &NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
            )
            .await?
        {
            Some(Term::Literal(v)) => v.value().to_owned(),
            _ => bail!("book component requires a title"),
        };
        index.insert(placement.clone(), parts.len());
        parts.push(Part {
            placement: placement.clone(),
            subject,
            parent,
            module,
            ontology,
            boundary,
            title,
        });
    }
    let mut has_module_child = HashSet::new();
    for p in &parts {
        if p.module &&
            let Some(parent) = &p.parent
        {
            has_module_child.insert(parent.clone());
        }
    }
    let mut owners: HashMap<NamedNode, NamedNode> = HashMap::new();
    let mut books: BTreeMap<NamedNode, Vec<NamedNode>> = BTreeMap::new();
    for p in &parts {
        let inherited = p
            .parent
            .as_ref()
            .and_then(|parent| owners.get(parent))
            .cloned();
        let owner = if let Some(owner) = inherited {
            ensure!(
                p.boundary != Some(true),
                "nested explicit book boundaries overlap at {}",
                p.subject
            );
            Some(owner)
        } else if (p.module &&
            p.boundary
                .unwrap_or(!has_module_child.contains(&p.placement))) ||
            p.ontology
        {
            Some(p.placement.clone())
        } else {
            None
        };
        if let Some(owner) = owner {
            owners.insert(p.placement.clone(), owner.clone());
            books.entry(owner).or_default().push(p.placement.clone());
        }
    }
    ensure!(
        !books.is_empty(),
        "bookshelf has no selected books"
    );
    for (root, members) in books {
        let p = &parts[index[&root]];
        let book = identity("book-plan", &[
            selection.plan.as_str(),
            root.as_str(),
        ]);
        for (s, predicate, value) in [
            (
                selection.plan.clone(),
                term("bookPlan"),
                Term::NamedNode(book.clone()),
            ),
            (
                book.clone(),
                NamedNode::new_unchecked(RDF_TYPE),
                term("BookPlan").into(),
            ),
            (book.clone(), term("bookRoot"), root.into()),
            (
                book.clone(),
                term("bookSubject"),
                p.subject.clone().into(),
            ),
            (
                book.clone(),
                NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
                Literal::new_simple_literal(&p.title).into(),
            ),
        ] {
            stream
                .write(Quad::new(
                    s,
                    predicate,
                    value,
                    selection.plan.clone(),
                ))
                .await?;
        }
        for placement in members {
            stream
                .write(Quad::new(
                    book.clone(),
                    term("bookPlacement"),
                    placement,
                    selection.plan.clone(),
                ))
                .await?;
        }
    }
    Ok(())
}
