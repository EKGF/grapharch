//! Bounded, explicitly rooted authored Markdown transclusions.
use {
    super::source::FileInput,
    anyhow::{Result, bail, ensure},
    oxrdf::{GraphName, Literal, NamedNode, Quad},
    sha2::{Digest, Sha256},
    std::{
        future::Future,
        path::{Component, Path, PathBuf},
        pin::Pin,
    },
    tokio::io::AsyncReadExt,
};

pub(super) struct Expansion {
    pub body:     String,
    pub metadata: Vec<Quad>,
    pub revision: String,
}
struct State {
    roots:    Vec<PathBuf>,
    active:   Vec<PathBuf>,
    metadata: Vec<Quad>,
    fragment: NamedNode,
    bytes:    usize,
    count:    usize,
    digests:  String,
}
impl Expansion {
    pub async fn read(
        input: &FileInput,
        root: Option<&Path>,
        extra: &[PathBuf],
        body: &str,
        local: bool,
    ) -> Result<Self> {
        let directives =
            body.contains("--8<--") || body.contains("{% include-markdown");
        if !directives {
            return Ok(Self {
                body:     body.into(),
                metadata: Vec::new(),
                revision: String::new(),
            });
        }
        ensure!(
            local,
            "includes require a pinned local source collector; supplied blobs \
             cannot read the working tree"
        );
        let mut roots = Vec::new(); // bounded: at most nine explicitly authorized roots.
        for root in
            std::iter::once(root.unwrap_or_else(|| {
                input.path.parent().unwrap_or(Path::new("."))
            }))
            .chain(extra.iter().map(PathBuf::as_path))
        {
            let canonical = tokio::fs::canonicalize(root).await?;
            if !roots.contains(&canonical) {
                roots.push(canonical);
            }
        }
        let source = tokio::fs::canonicalize(&input.path).await?;
        let mut state = State {
            roots,
            active: vec![source.clone()],
            metadata: Vec::new(),
            fragment: input.fragment_id(),
            bytes: 0,
            count: 0,
            digests: String::new(),
        };
        let body = state.expand(body, &source).await?;
        let revision = hex::encode(Sha256::digest(
            format!("{}:{body}", state.digests).as_bytes(),
        ));
        Ok(Self { body, metadata: state.metadata, revision })
    }
}
impl State {
    fn expand<'a>(
        &'a mut self,
        text: &'a str,
        source: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let mut output = String::new();
            let mut lines = text.split_inclusive('\n');
            let mut fence: Option<char> = None;
            while let Some(line) = lines.next() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    let marker = trimmed.chars().next().unwrap();
                    if fence == Some(marker) {
                        fence = None;
                    } else if fence.is_none() {
                        fence = Some(marker);
                    }
                    output.push_str(line);
                    continue;
                }
                if fence.is_some() ||
                    !(trimmed.starts_with("--8<--") ||
                        trimmed.starts_with("{% include-markdown"))
                {
                    output.push_str(line);
                    continue;
                }
                let indent = &line[..line.len() - trimmed.len()];
                let mut directive = trimmed.to_string();
                if directive.starts_with("{%") {
                    while !directive.contains("%}") {
                        let next = lines.next().ok_or_else(|| {
                            anyhow::anyhow!(
                                "unterminated include-markdown directive"
                            )
                        })?;
                        ensure!(
                            directive.len() + next.len() <= 4096,
                            "include directive exceeds 4096 bytes"
                        );
                        directive.push_str(next);
                    }
                }
                let (path, start, end) = parse(&directive)?;
                ensure!(
                    !Path::new(&path).is_absolute() &&
                        Path::new(&path).components().all(|c| {
                            matches!(
                                c,
                                Component::Normal(_) | Component::CurDir
                            )
                        }),
                    "include path must stay within an explicit root: {path}"
                );
                let mut selected = None;
                for root in &self.roots {
                    let candidate = root.join(&path);
                    match tokio::fs::canonicalize(&candidate).await {
                        Ok(canonical) => {
                            ensure!(
                                canonical.starts_with(root),
                                "include symlink escapes authorized root"
                            );
                            ensure!(
                                selected
                                    .as_ref()
                                    .is_none_or(|old| old == &canonical),
                                "ambiguous include path: {path}"
                            );
                            selected = Some(canonical);
                        },
                        Err(error)
                            if error.kind() == std::io::ErrorKind::NotFound => {
                        },
                        Err(error) => return Err(error.into()),
                    }
                }
                let path = selected.ok_or_else(|| {
                    anyhow::anyhow!("include source not found: {path}")
                })?;
                ensure!(
                    !self.active.contains(&path),
                    "include cycle at {}",
                    path.display()
                );
                ensure!(
                    self.active.len() < 16 && self.count < 128,
                    "include depth or dependency limit exceeded"
                );
                self.count += 1;
                let mut raw = String::new();
                tokio::fs::File::open(&path)
                    .await?
                    .take(262145)
                    .read_to_string(&mut raw)
                    .await?;
                ensure!(
                    raw.len() <= 262144,
                    "included source exceeds 256 KiB"
                );
                self.bytes += raw.len();
                ensure!(
                    self.bytes <= 1048576,
                    "include dependency bytes exceed 1 MiB"
                );
                let begin = marker(&raw, start.as_deref(), true)?;
                let finish = marker(&raw, end.as_deref(), false)?;
                ensure!(
                    begin <= finish,
                    "include end marker precedes start marker"
                );
                self.retain(&path, &raw, begin, finish);
                self.active.push(path.clone());
                let expanded = self.expand(&raw[begin..finish], &path).await?;
                self.active.pop();
                let expanded = rebase(&expanded, &path, source)?;
                for included in expanded.lines() {
                    output.push_str(indent);
                    output.push_str(included);
                    output.push('\n');
                }
                ensure!(
                    output.len() <= 1048576,
                    "expanded Markdown exceeds 1 MiB"
                );
            }
            ensure!(
                output.len() <= 1048576,
                "expanded Markdown exceeds 1 MiB"
            );
            Ok(output)
        })
    }

    fn retain(&mut self, path: &Path, raw: &str, begin: usize, end: usize) {
        let path = path.to_string_lossy();
        let digest = hex::encode(Sha256::digest(raw.as_bytes()));
        self.digests.push_str(&digest);
        let entity =
            crate::publication::dataset::identity("included-source", &[
                &path, &digest,
            ]);
        let selection =
            crate::publication::dataset::identity("included-selection", &[
                entity.as_str(),
                &begin.to_string(),
                &end.to_string(),
            ]);
        for (subject, predicate, value) in [
            (
                self.fragment.clone(),
                "http://www.w3.org/ns/prov#wasDerivedFrom",
                oxrdf::Term::NamedNode(selection.clone()),
            ),
            (
                selection.clone(),
                "http://www.w3.org/ns/prov#wasDerivedFrom",
                entity.clone().into(),
            ),
            (
                selection.clone(),
                "http://www.w3.org/ns/oa#start",
                Literal::from(raw[..begin].chars().count() as i64).into(),
            ),
            (
                selection,
                "http://www.w3.org/ns/oa#end",
                Literal::from(raw[..end].chars().count() as i64).into(),
            ),
            (
                entity.clone(),
                "http://purl.org/dc/terms/source",
                Literal::new_simple_literal(path.as_ref()).into(),
            ),
            (
                entity.clone(),
                "http://purl.org/dc/terms/identifier",
                Literal::new_simple_literal(digest).into(),
            ),
            (
                entity,
                "http://www.w3.org/ns/prov#value",
                Literal::new_simple_literal(raw).into(),
            ),
        ] {
            self.metadata.push(Quad::new(
                subject,
                NamedNode::new_unchecked(predicate),
                value,
                GraphName::DefaultGraph,
            ));
        }
    }
}
fn marker(raw: &str, marker: Option<&str>, start: bool) -> Result<usize> {
    let Some(marker) = marker else {
        return Ok(if start { 0 } else { raw.len() });
    };
    ensure!(!marker.is_empty(), "empty include marker");
    let mut matches = raw.match_indices(marker);
    let (offset, _) = matches
        .next()
        .ok_or_else(|| anyhow::anyhow!("include marker not found: {marker}"))?;
    ensure!(
        matches.next().is_none(),
        "ambiguous repeated include marker"
    );
    Ok(offset + if start { marker.len() } else { 0 })
}
fn parse(text: &str) -> Result<(String, Option<String>, Option<String>)> {
    let expression = regex::Regex::new(
        r#"(?s)^\{%\s*include-markdown\s+"([^"]+)"\s*(.*?)\s*%\}\s*$"#,
    )?;
    if let Some(c) = expression.captures(text) {
        let option = regex::Regex::new(r#"^\s*(start|end)="([^"]*)""#)?;
        let mut rest = c[2].trim();
        let mut start = None;
        let mut end = None;
        while !rest.is_empty() {
            let c = option.captures(rest).ok_or_else(|| {
                anyhow::anyhow!("unsupported include-markdown option: {rest}")
            })?;
            let target = if &c[1] == "start" {
                &mut start
            } else {
                &mut end
            };
            ensure!(target.is_none(), "duplicate include selector");
            *target = Some(c[2].into());
            rest = rest[c[0].len()..].trim();
        }
        return Ok((c[1].into(), start, end));
    }
    let snippet = regex::Regex::new(r#"^--8<--\s+"([^"]+)"\s*$"#)?;
    if let Some(c) = snippet.captures(text) {
        return Ok((c[1].into(), None, None));
    }
    bail!("unsupported include directive: {text}")
}
fn rebase(text: &str, from: &Path, to: &Path) -> Result<String> {
    let base = url::Url::from_file_path(from)
        .map_err(|_| anyhow::anyhow!("invalid include source path"))?;
    let target = url::Url::from_directory_path(to.parent().unwrap())
        .map_err(|_| anyhow::anyhow!("invalid including source path"))?;
    let mut edits = Vec::new(); // bounded: source text is limited to 256 KiB.
    for (event, range) in pulldown_cmark::Parser::new(text).into_offset_iter() {
        let destination = match event {
            pulldown_cmark::Event::Start(
                pulldown_cmark::Tag::Link { dest_url, .. } |
                pulldown_cmark::Tag::Image { dest_url, .. },
            ) => dest_url,
            _ => continue,
        };
        if destination.starts_with(['/', '#']) ||
            url::Url::parse(&destination).is_ok()
        {
            continue;
        }
        let joined = base.join(&destination)?;
        let relative = target
            .make_relative(&joined)
            .ok_or_else(|| anyhow::anyhow!("cannot rebase included link"))?;
        let original = &text[range.clone()];
        let needle = format!("]({destination}");
        if let Some(offset) = original.find(&needle) {
            let start = range.start + offset + 2;
            edits.push((start..start + destination.len(), relative));
        } else {
            bail!(
                "included relative reference links require inline link syntax"
            );
        }
    }
    let mut result = text.to_string();
    for (range, value) in edits.into_iter().rev() {
        result.replace_range(range, &value);
    }
    Ok(result)
}
