use {
    super::term,
    anyhow::{Result, bail, ensure},
    oxigraph::store::Store,
    oxrdf::{NamedNode, Term},
    std::path::PathBuf,
};

/// A supported output engine. Several targets can share one engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renderer {
    /// Static HTML without a server dependency.
    Html,
    /// Multipage static documentation website.
    Website,
    /// Standards-style static HTML with publication metadata.
    Specification,
    /// Component-sized standards pages with shared family navigation.
    SpecificationPages,
    /// Module books with separate websites and PDFs.
    Bookshelf,
    /// Typst source and PDF.
    Typst,
}

impl Renderer {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "html" => Ok(Self::Html),
            "website" => Ok(Self::Website),
            "specification" => Ok(Self::Specification),
            "specification-pages" => Ok(Self::SpecificationPages),
            "bookshelf" => Ok(Self::Bookshelf),
            "typst" => Ok(Self::Typst),
            other => bail!("unsupported renderer {other}"),
        }
    }
}

/// A target binds shared content to a scope, arrangement, and artifact
/// directory.
#[derive(Debug, Clone)]
pub struct Target {
    /// Stable target identity.
    pub id:          NamedNode,
    /// Reusable subject selection.
    pub scope:       NamedNode,
    /// Arrangement and presentation definition.
    pub profile:     NamedNode,
    /// Output engine.
    pub renderer:    Renderer,
    /// Target-owned output directory.
    pub destination: PathBuf,
}

impl Target {
    /// Read a validated target definition without loading other targets.
    pub fn read(store: &Store, id: NamedNode) -> Result<Self> {
        let scope = node(store, &id, "scope")?;
        let profile = node(store, &id, "profile")?;
        let renderer = Renderer::parse(&text(store, &id, "renderer")?)?;
        let destination = PathBuf::from(text(store, &id, "destination")?);
        ensure!(
            !destination.as_os_str().is_empty(),
            "{id} empty destination"
        );
        Ok(Self { id, scope, profile, renderer, destination })
    }
}

pub(crate) fn value(
    store: &Store,
    id: &NamedNode,
    property: &str,
) -> Result<Term> {
    let mut result = None;
    for quad in store.quads_for_pattern(
        Some(id.as_ref().into()),
        Some(term(property).as_ref()),
        None,
        None,
    ) {
        let object = quad?.object;
        if let Some(ref previous) = result {
            ensure!(previous == &object, "{id} conflicting {property}");
        } else {
            result = Some(object);
        }
    }
    result.ok_or_else(|| anyhow::anyhow!("{id} missing {property}"))
}

pub(crate) fn node(
    store: &Store,
    id: &NamedNode,
    property: &str,
) -> Result<NamedNode> {
    match value(store, id, property)? {
        Term::NamedNode(node) => Ok(node),
        _ => bail!("{id} {property} must be an IRI"),
    }
}

pub(crate) fn text(
    store: &Store,
    id: &NamedNode,
    property: &str,
) -> Result<String> {
    match value(store, id, property)? {
        Term::Literal(literal) => Ok(literal.value().to_owned()),
        _ => bail!("{id} {property} must be a literal"),
    }
}

/// Stream distinct logical values across default and named contribution graphs.
pub(crate) fn objects(
    store: &Store,
    subject: &NamedNode,
    property: &str,
) -> Result<impl Iterator<Item = Result<Term>>> {
    let query = format!(
        "SELECT DISTINCT ?object WHERE {{ {{ {subject} {} ?object }} UNION {{ \
         GRAPH ?graph {{ {subject} {} ?object }} }} }}",
        term(property),
        term(property)
    );
    let oxigraph::sparql::QueryResults::Solutions(rows) =
        oxigraph::sparql::SparqlEvaluator::new()
            .parse_query(query.as_str())?
            .on_store(store)
            .execute()?
    else {
        bail!("expected property values")
    };
    Ok(rows.map(|row| {
        row?.get("object")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing property value"))
    }))
}
