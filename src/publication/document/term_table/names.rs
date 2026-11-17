//! How a term table names resources: terms of the documented ontology by
//! local name, others by the family's namespace prefixes.
use {
    super::super::annotated_term::escape,
    oxrdf::NamedNode,
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

/// Resource names relative to one documented ontology namespace.
#[derive(Clone)]
pub(super) struct TermNames {
    /// Namespace to prefix, as bound by the family.
    prefixes:   Arc<BTreeMap<String, String>>,
    namespace:  String,
    documented: Arc<HashSet<NamedNode>>,
}

impl TermNames {
    pub fn new(
        prefixes: Arc<BTreeMap<String, String>>,
        namespace: String,
        documented: Arc<HashSet<NamedNode>>,
    ) -> Self {
        Self { prefixes, namespace, documented }
    }

    fn local<'a>(iri: &'a str, namespace: &str) -> Option<&'a str> {
        iri.strip_prefix(namespace)
            .filter(|local| !local.is_empty() && !local.contains(['/', '#']))
    }

    /// The prefixed name under the longest bound namespace, or the IRI.
    pub fn curie(&self, iri: &NamedNode) -> String {
        self.prefixes
            .iter()
            .filter_map(|(namespace, prefix)| {
                Self::local(iri.as_str(), namespace)
                    .map(|local| (namespace.len(), prefix, local))
            })
            .max_by_key(|(length, _, _)| *length)
            .map(|(_, prefix, local)| format!("{prefix}:{local}"))
            .unwrap_or_else(|| iri.as_str().into())
    }

    /// The local name of an own term, otherwise the prefixed name.
    pub fn name(&self, iri: &NamedNode) -> String {
        Self::local(iri.as_str(), &self.namespace)
            .map(str::to_owned)
            .unwrap_or_else(|| self.curie(iri))
    }

    /// Inert Markdown for a name, linked when the family documents it.
    pub fn markdown(&self, iri: &NamedNode) -> String {
        let name = escape(&self.name(iri));
        if self.documented.contains(iri) {
            format!("[{name}]({})", iri.as_str())
        } else {
            name
        }
    }

    /// A resource value: a documented term links to its entry, an HTTP(S)
    /// IRI is an external link, and any other IRI is named.
    pub fn value(&self, iri: &NamedNode) -> String {
        if self.documented.contains(iri) {
            return self.markdown(iri);
        }
        if (iri.as_str().starts_with("https://") ||
            iri.as_str().starts_with("http://")) &&
            self.curie(iri) == iri.as_str()
        {
            return format!("[{}]({})", escape(iri.as_str()), iri.as_str());
        }
        escape(&self.name(iri))
    }
}
