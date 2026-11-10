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
    /// Resolve a supported renderer definition by its exact RDF identity.
    pub fn from_node(value: &NamedNode) -> Result<Self> {
        match value.as_str() {
            "https://ekgf.org/ontology/grapharch#HtmlRenderer" => Ok(Self::Html),
            "https://ekgf.org/ontology/grapharch#WebsiteRenderer" => Ok(Self::Website),
            "https://ekgf.org/ontology/grapharch#SpecificationRenderer" => Ok(Self::Specification),
            "https://ekgf.org/ontology/grapharch#SpecificationPagesRenderer" => Ok(Self::SpecificationPages),
            "https://ekgf.org/ontology/grapharch#BookshelfRenderer" => Ok(Self::Bookshelf),
            "https://ekgf.org/ontology/grapharch#TypstRenderer" => Ok(Self::Typst),
            other => bail!("unsupported renderer definition {other}"),
        }
    }

    /// Stable identity of this shared renderer definition.
    pub fn iri(self) -> NamedNode {
        term(match self {
            Self::Html => "HtmlRenderer",
            Self::Website => "WebsiteRenderer",
            Self::Specification => "SpecificationRenderer",
            Self::SpecificationPages => "SpecificationPagesRenderer",
            Self::Bookshelf => "BookshelfRenderer",
            Self::Typst => "TypstRenderer",
        })
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Website => "website",
            Self::Specification => "specification",
            Self::SpecificationPages => "specification-pages",
            Self::Bookshelf => "bookshelf",
            Self::Typst => "typst",
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
        let renderer = Renderer::from_node(&node(store, &id, "renderer")?)?;
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
