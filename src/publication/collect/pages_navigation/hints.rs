//! Bounded reading of .pages.yaml and its directory membership.
use {
    anyhow::{Result, ensure},
    serde::Deserialize,
    sha2::{Digest, Sha256},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::{Component, Path, PathBuf},
    },
};
#[derive(Default, Deserialize)]
struct Hints {
    title: Option<String>,
    #[serde(default)]
    hide:  bool,
    nav:   Option<Vec<Item>>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Item {
    Path(String),
    Label(BTreeMap<String, String>),
}
pub(super) struct Entry {
    pub path:  PathBuf,
    pub label: Option<String>,
}
pub(super) struct Directory {
    pub source:   String,
    pub body:     String,
    pub revision: String,
    pub title:    Option<String>,
    pub hidden:   bool,
    pub items:    Vec<Entry>,
}
impl Directory {
    pub async fn read(root: &Path, directory: &Path) -> Result<Self> {
        let file = directory.join(".pages.yaml");
        let mut source = super::super::page::source_path(root, &file)?;
        let (mut body, hints, explicit) =
            match tokio::fs::symlink_metadata(&file).await {
                Ok(metadata) => {
                    ensure!(
                        metadata.is_file(),
                        "navigation hints must be a regular file"
                    );
                    let input = super::super::source::FileInput::new(
                        "urn:grapharch:navigation-read",
                        "urn:grapharch:navigation-read",
                        file,
                    )?;
                    let (body, _) = input.authored().await?;
                    let hints = serde_yaml::from_str::<Hints>(&body)?;
                    (body, hints, true)
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    (String::new(), Hints::default(), false)
                },
                Err(error) => return Err(error.into()),
            };
        let mut available = BTreeSet::new(); // bounded: 512 entries per directory.
        let mut entries = tokio::fs::read_dir(directory).await?;
        let mut visited = 0;
        while let Some(entry) = entries.next_entry().await? {
            visited += 1;
            ensure!(
                visited <= 4096,
                "navigation discovery exceeds 4096 directory entries"
            );
            let name = entry
                .file_name()
                .to_str()
                .ok_or_else(|| {
                    anyhow::anyhow!("navigation names must be UTF-8")
                })?
                .to_owned();
            if name.starts_with(['.', '_']) {
                continue;
            }
            let kind = entry.file_type().await?;
            if kind.is_dir() ||
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "md")
            {
                ensure!(!kind.is_symlink(), "navigation rejects symlinks");
                ensure!(
                    available.len() < 512,
                    "navigation directory exceeds 512 items"
                );
                available.insert(name);
            }
        }
        if !explicit {
            source = format!(
                "{}/",
                super::super::page::source_path(root, directory)?
            );
            body = available.iter().map(|name| format!("{name}\n")).collect(); // bounded: 512 directory entries.
        }
        let mut digest = Sha256::new();
        digest.update(body.as_bytes());
        for name in &available {
            digest.update(name.as_bytes());
            digest.update([0]);
        }
        let nav = hints.nav.unwrap_or_else(|| {
            available.iter().cloned().map(Item::Path).collect()
        });
        ensure!(
            nav.len() <= 512,
            "navigation exceeds 512 authored items"
        );
        let mut authored = Vec::new(); // bounded: 512 authored items.
        let mut explicit_paths = BTreeSet::new(); // bounded: 512 authored items.
        for item in nav {
            let (path, label) = match item {
                Item::Path(path) => (path, None),
                Item::Label(labels) => {
                    ensure!(
                        labels.len() == 1,
                        "navigation label must map to exactly one path"
                    );
                    let (label, path) = labels.into_iter().next().unwrap();
                    (path, Some(label))
                },
            };
            ensure!(
                explicit_paths.insert(path.clone()),
                "duplicate directory navigation entry {path}"
            );
            authored.push((path, label));
        }
        let mut items = Vec::new(); // bounded: 512 paths.
        for (path, label) in authored {
            if path == "..." {
                for name in &available {
                    if !explicit_paths.contains(name) {
                        items.push(Entry {
                            path:  directory.join(name),
                            label: None,
                        });
                    }
                }
                continue;
            }
            ensure!(
                !path.is_empty() &&
                    Path::new(&path)
                        .components()
                        .all(|part| matches!(part, Component::Normal(_))),
                "navigation path must be relative without traversal"
            );
            ensure!(
                tokio::fs::try_exists(directory.join(&path)).await?,
                "navigation source does not exist: {path}"
            );
            items.push(Entry { path: directory.join(path), label });
        }
        ensure!(
            items.len() <= 512,
            "navigation expansion exceeds 512 items"
        );
        Ok(Self {
            source,
            body,
            revision: hex::encode(digest.finalize()),
            title: hints.title,
            hidden: hints.hide,
            items,
        })
    }
}
