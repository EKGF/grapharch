//! Token values of a resolved theme, inherited along its lineage.
use {
    super::{
        Builtin,
        CoverBlock,
        Lineage,
        tokens::{TOKENS, key, token},
        values::{boolean, hex, integer},
    },
    crate::publication::{NAMESPACE, RDF_TYPE, view::RevisionView},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, HashSet},
};

/// A complete, validated PDF house style.
pub(crate) struct PdfTheme {
    lineage: Lineage,
    // bounded: one value per token of the fixed vocabulary.
    values:  BTreeMap<&'static str, Term>,
    // bounded: at most 32 cover blocks.
    cover:   Vec<CoverBlock>,
}

const MAX_STATEMENTS: usize = 256;
const MAX_COVER_BLOCKS: usize = 32;

impl PdfTheme {
    /// Resolve the profile's theme. A more specific theme overrides tokens of
    /// its bases; the most specific theme declaring cover blocks supplies the
    /// whole cover.
    pub(crate) async fn read(
        view: &RevisionView,
        profile: &NamedNode,
        default: Builtin,
    ) -> Result<Self> {
        let lineage = Lineage::read(view, profile, default).await?;
        let mut values = BTreeMap::new();
        let mut cover_blocks: Option<Vec<NamedNode>> = None;
        for theme in lineage.themes() {
            let mut declared = HashSet::new();
            let mut blocks = Vec::new();
            let mut rows = view.statements(theme).await?;
            let mut count = 0;
            while let Some(row) = rows.next().await? {
                count += 1;
                ensure!(
                    count <= MAX_STATEMENTS,
                    "theme {theme} exceeds {MAX_STATEMENTS} statements"
                );
                let Term::NamedNode(predicate) = &row["predicate"] else {
                    bail!("theme predicate must be an IRI")
                };
                if predicate.as_str() == RDF_TYPE {
                    continue;
                }
                let Some(local) = predicate.as_str().strip_prefix(NAMESPACE)
                else {
                    continue;
                };
                match local {
                    "baseTheme" => {},
                    "coverBlock" => {
                        let Term::NamedNode(block) = &row["value"] else {
                            bail!("theme {theme} ga:coverBlock must be an IRI")
                        };
                        ensure!(
                            blocks.len() < MAX_COVER_BLOCKS,
                            "theme {theme} exceeds {MAX_COVER_BLOCKS} cover \
                             blocks"
                        );
                        blocks.push(block.clone());
                    },
                    _ => {
                        let token = token(local).ok_or_else(|| {
                            anyhow::anyhow!(
                                "theme {theme} declares unsupported token \
                                 ga:{local}"
                            )
                        })?;
                        ensure!(
                            declared.insert(token.name),
                            "theme {theme} declares ga:{local} more than once"
                        );
                        values
                            .entry(token.name)
                            .or_insert_with(|| row["value"].clone());
                    },
                }
            }
            if cover_blocks.is_none() && !blocks.is_empty() {
                cover_blocks = Some(blocks);
            }
        }
        let selected = lineage.selected().clone();
        for token in TOKENS {
            match values.get(token.name) {
                Some(value) => {
                    token.kind.typst(token.name, value)?;
                },
                None if token.required => {
                    bail!(
                        "theme {selected} and its bases do not declare \
                         required token ga:{}",
                        token.name
                    )
                },
                None => {},
            }
        }
        let mut cover = Vec::new();
        for block in cover_blocks.unwrap_or_default() {
            cover.push(CoverBlock::read(view, &block).await?);
        }
        ensure!(
            !cover.is_empty(),
            "theme {selected} and its bases declare no cover blocks"
        );
        cover.sort_by_key(|block| block.order);
        ensure!(
            cover.windows(2).all(|pair| pair[0].order != pair[1].order),
            "theme {selected} has ambiguous cover block orders"
        );
        let theme = Self { lineage, values, cover };
        hex(
            "accent",
            &theme.text("accent").unwrap_or_default(),
        )?;
        if theme.flag("annexRectoStart") {
            ensure!(
                theme.text("blankPageText").is_some(),
                "theme {selected} starts annexes on recto pages and requires \
                 ga:blankPageText"
            );
        }
        if let Some(label) = theme.text("annexLabel") {
            ensure!(
                label.contains("{title}"),
                "theme {selected} ga:annexLabel must contain {{title}}"
            );
        }
        Ok(theme)
    }

    pub(crate) fn cover(&self) -> &[CoverBlock] { &self.cover }

    /// A validated boolean token.
    pub(crate) fn flag(&self, name: &str) -> bool {
        self.values
            .get(name)
            .and_then(|value| boolean(name, value).ok())
            .unwrap_or(false)
    }

    /// The lexical value of a literal token, when declared.
    pub(crate) fn text(&self, name: &str) -> Option<String> {
        match self.values.get(name) {
            Some(Term::Literal(value)) => Some(value.value().to_owned()),
            _ => None,
        }
    }

    pub(crate) fn depth(&self, name: &str) -> Result<usize> {
        let value = self
            .values
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("theme lacks ga:{name}"))?;
        Ok(integer(name, value)? as usize)
    }

    /// The `#let ga-theme` dictionary that the shared Typst theme reads.
    pub(crate) fn typst(&self) -> Result<String> {
        let mut fields = Vec::with_capacity(TOKENS.len());
        for token in TOKENS {
            let value = match self.values.get(token.name) {
                Some(value) => token.kind.typst(token.name, value)?,
                None => "none".into(),
            };
            fields.push(format!("  {}: {value},", key(token.name)));
        }
        Ok(format!(
            "// Theme: {}\n#let ga-theme = (\n{}\n)\n",
            self.lineage.selected().as_str(),
            fields.join("\n")
        ))
    }
}
