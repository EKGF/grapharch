//! The MkDocs presentation mapping table, `mkdocs-import.ttl`: which GraphArch
//! theme and screen tokens present an MkDocs theme or plugin, and the values
//! of Material's named palette colours. The table is RDF; this module only
//! reads it.
use {
    crate::publication::{NAMESPACE, RDF_TYPE, term},
    anyhow::{Context, Result, ensure},
    oxrdf::{NamedNode, NamedOrBlankNode, Term},
    oxrdfio::{RdfFormat, RdfParser},
    std::collections::BTreeMap,
};

const TABLE: &str =
    include_str!("../../../../docs/other/ontology/mkdocs-import.ttl");
/// bounded: the compile-time table holds a few dozen entries.
const MAX_ENTRIES: usize = 256;

/// What the table maps one MkDocs name to.
#[derive(Default)]
struct Entry {
    theme_name:   Option<String>,
    plugin_name:  Option<String>,
    mapped_theme: Option<NamedNode>,
    // bounded: the statements of one table entry.
    tokens:       Vec<(NamedNode, Term)>,
    color_name:   Option<String>,
    screen_color: Option<String>,
}

/// The parsed mapping table.
pub(super) struct Mappings {
    // bounded: at most MAX_ENTRIES entries.
    entries: Vec<Entry>,
}

/// The presentation an MkDocs configuration selects through the table.
pub(super) struct Selection {
    pub theme:  Option<NamedNode>,
    // bounded: the tokens of the matching table entries.
    pub tokens: Vec<(NamedNode, Term)>,
}

impl Mappings {
    pub(super) fn embedded() -> Result<Self> {
        let mut by_subject: BTreeMap<NamedNode, Entry> = BTreeMap::new();
        let mut types: BTreeMap<NamedNode, NamedNode> = BTreeMap::new();
        for quad in RdfParser::from_format(RdfFormat::Turtle)
            .for_slice(TABLE.as_bytes())
        {
            let quad = quad.context("MkDocs mapping table is valid Turtle")?;
            let NamedOrBlankNode::NamedNode(subject) = quad.subject else {
                continue;
            };
            if subject.as_str().starts_with(NAMESPACE) {
                continue; // term declarations, not table entries
            }
            if quad.predicate.as_str() == RDF_TYPE {
                if let Term::NamedNode(class) = quad.object {
                    types.insert(subject, class);
                }
                continue;
            }
            let entry = by_subject.entry(subject).or_default();
            let text = || {
                match &quad.object {
                    Term::Literal(literal) => Some(literal.value().to_owned()),
                    _ => None,
                }
            };
            match quad.predicate.as_str().strip_prefix(NAMESPACE) {
                Some("mkdocsTheme") => entry.theme_name = text(),
                Some("mkdocsPlugin") => entry.plugin_name = text(),
                Some("mkdocsColor") => entry.color_name = text(),
                Some("screenColor") => entry.screen_color = text(),
                Some("mappedTheme") => {
                    if let Term::NamedNode(theme) = &quad.object {
                        entry.mapped_theme = Some(theme.clone());
                    }
                },
                Some(_) => entry.tokens.push((quad.predicate, quad.object)),
                None => {},
            }
            ensure!(
                by_subject.len() <= MAX_ENTRIES,
                "MkDocs mapping table exceeds {MAX_ENTRIES} entries"
            );
        }
        for (subject, class) in &types {
            ensure!(
                *class == term("MkDocsMapping") ||
                    *class == term("MaterialColor"),
                "mapping table entry {subject} has unknown class {class}"
            );
        }
        Ok(Self { entries: by_subject.into_values().collect() })
    }

    /// The theme and tokens for a configuration of theme `theme_name` listing
    /// `plugins` in order. A plugin's theme wins over the theme's.
    pub(super) fn select(
        &self,
        theme_name: Option<&str>,
        plugins: &[String],
    ) -> Selection {
        let by_theme = self.entries.iter().filter(|entry| {
            entry.theme_name.is_some() &&
                entry.theme_name.as_deref() == theme_name
        });
        let by_plugin = plugins.iter().flat_map(|plugin| {
            self.entries.iter().filter(move |entry| {
                entry.plugin_name.as_deref() == Some(plugin.as_str())
            })
        });
        let mut selection = Selection { theme: None, tokens: Vec::new() };
        for entry in by_plugin.chain(by_theme) {
            if selection.theme.is_none() {
                selection.theme.clone_from(&entry.mapped_theme);
            }
            for (predicate, value) in &entry.tokens {
                if !selection.tokens.iter().any(|(known, _)| known == predicate)
                {
                    selection.tokens.push((predicate.clone(), value.clone()));
                }
            }
        }
        selection
    }

    /// The screen colour of a Material palette colour name such as
    /// `deep orange` or `deep-orange`.
    pub(super) fn color(&self, name: &str) -> Option<&str> {
        let name = name.trim().to_lowercase().replace('-', " ");
        self.entries.iter().find_map(|entry| {
            (entry.color_name.as_deref() == Some(name.as_str()))
                .then_some(entry.screen_color.as_deref())
                .flatten()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Mappings;

    #[test]
    fn material_ekgf_maps_to_the_ekgf_documentation_theme() {
        let table = Mappings::embedded().unwrap();
        let selection = table.select(Some("material"), &[
            "search".into(),
            "material-ekgf".into(),
        ]);
        assert_eq!(
            selection.theme.unwrap(),
            crate::publication::term("EkgfDocumentationTheme")
        );
        assert!(
            selection
                .tokens
                .iter()
                .any(|(p, _)| p == &crate::publication::term("contentWidth"))
        );
        assert!(table.select(Some("mkdocs"), &[]).theme.is_none());
        assert_eq!(table.color("deep-orange"), Some("#ff6e42"));
        assert_eq!(table.color("Indigo"), Some("#4051b5"));
        assert_eq!(table.color("custom"), None);
    }
}
