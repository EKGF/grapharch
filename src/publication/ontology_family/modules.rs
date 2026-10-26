//! Resolve only explicit, collected module memberships; directories and import
//! dependencies never invent containment. Unassigned ontologies remain visible.
use super::*;

pub(super) struct Chapter {
    pub subject:     NamedNode,
    pub parent:      Option<NamedNode>,
    pub title:       String,
    pub module_body: Option<String>,
    pub member:      Option<usize>,
    pub depth:       usize,
}
struct Module {
    title:       String,
    description: String,
    parts:       Vec<NamedNode>,
}
pub(super) async fn chapters(
    dataset: &Dataset,
    view: &RevisionView,
    members: &[Member],
) -> Result<Vec<Chapter>> {
    let pattern = view.pattern(
        &format!("?module a {}", term("OntologyModule")),
        0,
    );
    // Use the snapshot's dataset, never the source filesystem.
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?module WHERE {{ {pattern} }}"
        ))
        .await?;
    let mut modules = BTreeMap::new();
    while let Some(row) = rows.next().await? {
        ensure!(
            modules.len() < 4096,
            "family exceeds module budget"
        );
        let Term::NamedNode(module) = &row["module"] else {
            anyhow::bail!("module must have a named identity")
        };
        let labels = english(
            view,
            module,
            "http://www.w3.org/2000/01/rdf-schema#label",
        )
        .await?;
        ensure!(
            labels.len() == 1 && !labels[0].trim().is_empty(),
            "module {module} requires one English label"
        );
        let mut description = String::new();
        for property in [
            "http://purl.org/dc/terms/abstract",
            "http://purl.org/dc/terms/description",
            "http://www.w3.org/2000/01/rdf-schema#comment",
        ] {
            let values = english(view, module, property).await?;
            ensure!(
                values.len() <= 1,
                "module {module} has ambiguous descriptions"
            );
            if let Some(value) = values.first() {
                description = plain(value);
                break;
            }
        }
        let mut children = view.objects(module, "moduleMember").await?;
        let mut parts = Vec::new();
        while let Some(row) = children.next().await? {
            ensure!(parts.len() < 4096, "module exceeds member budget");
            let Term::NamedNode(part) = &row["value"] else {
                anyhow::bail!("module member must be an IRI")
            };
            parts.push(part.clone());
        }
        modules.insert(module.clone(), Module {
            title: labels[0].clone(),
            description,
            parts,
        });
    }
    let ontology_index = members
        .iter()
        .enumerate()
        .map(|(i, m)| (m.ontology.clone(), i))
        .collect::<BTreeMap<_, _>>();
    let mut parents = BTreeMap::new();
    for (id, module) in &modules {
        for part in &module.parts {
            ensure!(
                modules.contains_key(part) || ontology_index.contains_key(part),
                "module {id} references an uncollected member {part}"
            );
            ensure!(
                parents.insert(part.clone(), id.clone()).is_none(),
                "module membership has multiple parents for {part}; an \
                 explicit publication arrangement is required"
            );
        }
    }
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut roots = modules
        .keys()
        .filter(|id| !parents.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    roots.sort_by_key(|id| (modules[id].title.clone(), id.clone()));
    for root in roots {
        walk(
            &root,
            None,
            1,
            &modules,
            &ontology_index,
            members,
            &mut seen,
            &mut result,
        )?;
    }
    ensure!(
        seen.len() == modules.len(),
        "module membership contains a cycle"
    );
    for (i, member) in members.iter().enumerate() {
        if !parents.contains_key(&member.ontology) {
            result.push(Chapter {
                subject:     member.ontology.clone(),
                parent:      None,
                title:       member.title.clone(),
                module_body: None,
                member:      Some(i),
                depth:       1,
            });
        }
    }
    Ok(result)
}
#[allow(clippy::too_many_arguments)]
fn walk(
    id: &NamedNode,
    parent: Option<NamedNode>,
    depth: usize,
    modules: &BTreeMap<NamedNode, Module>,
    ontologies: &BTreeMap<NamedNode, usize>,
    members: &[Member],
    seen: &mut HashSet<NamedNode>,
    out: &mut Vec<Chapter>,
) -> Result<()> {
    ensure!(
        depth <= 8,
        "module hierarchy exceeds eight publication levels"
    );
    ensure!(
        seen.insert(id.clone()),
        "module membership contains a cycle at {id}"
    );
    let module = &modules[id];
    let mut parts = module.parts.clone();
    let title = |part: &NamedNode| {
        modules
            .get(part)
            .map(|m| m.title.clone())
            .unwrap_or_else(|| members[ontologies[part]].title.clone())
    };
    parts.sort_by_key(|part| (title(part), part.clone()));
    let mut body = format!(
        "{}\n\n**Module IRI:** `{}`\n\n",
        module.description,
        id.as_str()
    );
    for part in &parts {
        body.push_str(&format!(
            "- [{}]({})\n",
            plain(&title(part)),
            part.as_str()
        ));
    }
    out.push(Chapter {
        subject: id.clone(),
        parent,
        title: module.title.clone(),
        module_body: Some(body),
        member: None,
        depth,
    });
    for part in parts {
        if modules.contains_key(&part) {
            walk(
                &part,
                Some(id.clone()),
                depth + 1,
                modules,
                ontologies,
                members,
                seen,
                out,
            )?;
        } else {
            let i = ontologies[&part];
            out.push(Chapter {
                subject:     part,
                parent:      Some(id.clone()),
                title:       members[i].title.clone(),
                module_body: None,
                member:      Some(i),
                depth:       depth + 1,
            });
        }
    }
    Ok(())
}
fn plain(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' | '#'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
