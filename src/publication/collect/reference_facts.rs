//! Find every reference an authored Markdown source makes and record it as
//! RDF at collection time: Markdown links and images, HTML `href`, `src`,
//! `srcset` and `<object data>`, and `!include` lines in diagram fences.
//!
//! A relative reference to a file outside the collected pages is checked
//! against the repository: the Git index at the build commit, or the file
//! tree of a copy without Git. The renderer's classifier then knows whether
//! a repository-file link is real or broken without reading sources.
use {
    super::source::FileInput,
    crate::publication::{dataset::identity, reference::ReferenceForm, term},
    anyhow::Result,
    oxrdf::{Literal, Quad},
    pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd},
    std::{
        collections::BTreeSet,
        path::{Component, Path, PathBuf},
    },
};

/// Most references recorded per source.
const MAX_REFERENCES: usize = 4096;
/// Fence languages whose `!include` lines name other files.
const DIAGRAM_LANGUAGES: &[&str] =
    &["plantuml", "puml", "mermaid", "dot", "graphviz"];

/// Where in the source a reference was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ReferenceContext {
    Link,
    Image,
    Html,
    DiagramInclude,
}

impl ReferenceContext {
    fn term(self) -> &'static str {
        match self {
            Self::Link => "LinkContext",
            Self::Image => "ImageContext",
            Self::Html => "HtmlContext",
            Self::DiagramInclude => "DiagramIncludeContext",
        }
    }
}

/// Every reference in `body`, in source order, deduplicated.
pub(crate) fn find(body: &str) -> Vec<(ReferenceContext, String)> {
    let mut found = BTreeSet::new(); // bounded: MAX_REFERENCES.
    let mut ordered = Vec::new(); // bounded: MAX_REFERENCES.
    let mut push = |context: ReferenceContext, value: &str| {
        let value = value.trim();
        if !value.is_empty() &&
            ordered.len() < MAX_REFERENCES &&
            found.insert((context, value.to_owned()))
        {
            ordered.push((context, value.to_owned()));
        }
    };
    let attribute = regex::Regex::new(
        r#"(?i)\b(href|src|data|srcset)\s*=\s*["']([^"']*)["']"#,
    )
    .expect("valid attribute pattern");
    let mut diagram = false;
    for event in Parser::new_ext(body, crate::publication::markdown::options())
    {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                push(ReferenceContext::Link, &dest_url)
            },
            Event::Start(Tag::Image { dest_url, .. }) => {
                push(ReferenceContext::Image, &dest_url)
            },
            Event::Html(html) | Event::InlineHtml(html) => {
                for capture in attribute.captures_iter(&html) {
                    if capture[1].eq_ignore_ascii_case("srcset") {
                        for candidate in capture[2].split(',') {
                            if let Some(url) =
                                candidate.split_whitespace().next()
                            {
                                push(ReferenceContext::Html, url);
                            }
                        }
                    } else {
                        push(ReferenceContext::Html, &capture[2]);
                    }
                }
            },
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language))) => {
                diagram = DIAGRAM_LANGUAGES.contains(
                    &language.split_whitespace().next().unwrap_or_default(),
                );
            },
            Event::End(TagEnd::CodeBlock) => diagram = false,
            Event::Text(text) if diagram => {
                for line in text.lines() {
                    let line = line.trim();
                    if let Some(rest) = line
                        .strip_prefix("!include ")
                        .or_else(|| line.strip_prefix("!includeurl "))
                        .or_else(|| line.strip_prefix("!include_once "))
                    {
                        push(ReferenceContext::DiagramInclude, rest);
                    }
                }
            },
            _ => {},
        }
    }
    ordered
}

/// Whether repository paths exist at the build's commit.
pub(super) struct RepositoryFiles {
    /// Work-tree root and the paths Git tracks, when the root is a checkout.
    git: Option<(PathBuf, gix::index::File)>,
}

impl RepositoryFiles {
    pub(super) fn open(root: &Path) -> Self {
        let git = root
            .ancestors()
            .find(|directory| directory.join(".git").exists())
            .and_then(|directory| {
                let repo = gix::open(directory).ok()?;
                let index = repo.index_or_empty().ok()?;
                Some((directory.to_owned(), index.into_owned_or_cloned()))
            });
        Self { git }
    }

    /// The directory references may not leave: the work tree, else `root`.
    fn boundary<'a>(&'a self, root: &'a Path) -> &'a Path {
        self.git
            .as_ref()
            .map_or(root, |(directory, _)| directory.as_path())
    }

    /// Whether `path`, an absolute normalized path, is part of the repository.
    pub(crate) fn exists(&self, path: &Path) -> bool {
        match &self.git {
            Some((root, index)) => {
                let Ok(relative) = path.strip_prefix(root) else {
                    return false
                };
                let relative = relative.to_string_lossy().replace('\\', "/");
                index.entry_by_path(relative.as_str().into()).is_some() ||
                    index
                        .prefixed_entries(
                            format!("{relative}/").as_str().into(),
                        )
                        .is_some_and(|entries| !entries.is_empty())
            },
            None => path.exists(),
        }
    }
}

/// Resolve `value`, written in `source`, to an absolute path below `root`;
/// `None` for anything that is not a local relative path or leaves `root`.
fn local_path(root: &Path, source: &Path, value: &str) -> Option<PathBuf> {
    let ReferenceForm::Relative { path, .. } = ReferenceForm::parse(value)
    else {
        return None;
    };
    if path.is_empty() {
        return None;
    }
    let decoded = percent_encoding::percent_decode_str(&path)
        .decode_utf8()
        .ok()?;
    let mut resolved = source.parent()?.to_owned();
    for part in Path::new(decoded.as_ref()).components() {
        match part {
            Component::Normal(part) => resolved.push(part),
            Component::ParentDir => {
                resolved.pop();
            },
            Component::CurDir => {},
            _ => return None,
        }
    }
    resolved.starts_with(root).then_some(resolved)
}

/// The `ga:Reference` records of one source.
pub(super) fn describe(
    input: &FileInput,
    root: &Path,
    relative: &str,
    body: &str,
    files: &RepositoryFiles,
) -> Result<Vec<Quad>> {
    let mut quads = Vec::new(); // bounded: MAX_REFERENCES references.
    for (context, value) in find(body) {
        let node = identity("reference", &[relative, &value]);
        quads.extend([
            input.quad(&node, "type", term("Reference")),
            input.quad(
                &node,
                "referenceSource",
                Literal::new_simple_literal(relative),
            ),
            input.quad(
                &node,
                "originalReference",
                Literal::new_simple_literal(&value),
            ),
            input.quad(
                &node,
                "referenceForm",
                term(ReferenceForm::parse(&value).term()),
            ),
            input.quad(&node, "referenceContext", term(context.term())),
            input.quad(&input.fragment_id(), "reference", node.clone()),
        ]);
        if let Some(path) =
            local_path(files.boundary(root), &input.path, &value)
        {
            quads.push(input.quad(
                &node,
                "repositoryFileExists",
                Literal::from(files.exists(&path)),
            ));
        }
    }
    Ok(quads)
}

#[cfg(test)]
mod tests {
    use super::{ReferenceContext, find};

    #[test]
    fn every_reference_syntax_is_found() {
        let body = "[a](a.md) ![i](i.png) [r][ref]\n\n[ref]: https://x.example/\n\n\
            <p><a href=\"../b.json\">b</a><img src=\"c.svg\" srcset=\"c1.png 1x, c2.png 2x\">\
            <object data=\"d.svg\"></object></p>\n\n```plantuml\n@startuml\n!include common.puml\n\
            @enduml\n```\n";
        let found = find(body);
        let expected = [
            (ReferenceContext::Link, "a.md"),
            (ReferenceContext::Image, "i.png"),
            (ReferenceContext::Link, "https://x.example/"),
            (ReferenceContext::Html, "../b.json"),
            (ReferenceContext::Html, "c.svg"),
            (ReferenceContext::Html, "c1.png"),
            (ReferenceContext::Html, "c2.png"),
            (ReferenceContext::Html, "d.svg"),
            (ReferenceContext::DiagramInclude, "common.puml"),
        ];
        for (context, value) in expected {
            assert!(
                found.iter().any(|(c, v)| *c == context && v == value),
                "{context:?} {value} in {found:?}"
            );
        }
    }
}
