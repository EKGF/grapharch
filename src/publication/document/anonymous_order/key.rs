//! Content-derived ordering keys of RDF values.
use {
    anyhow::{Result, ensure},
    oxrdf::Term,
    std::cmp::Ordering,
};

/// The explicit `sh:order` of an anonymous value.
#[derive(Clone, Copy, Debug)]
pub(super) struct ShaclOrder(f64);

impl ShaclOrder {
    pub(super) fn try_new(value: &Term) -> Result<Self> {
        let Term::Literal(literal) = value else {
            anyhow::bail!("sh:order must be a numeric literal")
        };
        let number: f64 = literal.value().parse().map_err(|_| {
            anyhow::anyhow!("sh:order {} is not numeric", literal.value())
        })?;
        ensure!(number.is_finite(), "sh:order must be finite");
        Ok(Self(number))
    }
}

/// Content-derived presentation key of one anonymous value.
#[derive(Debug)]
pub(super) struct ContentKey {
    pub(super) order:  Option<ShaclOrder>,
    pub(super) label:  Option<String>,
    pub(super) digest: [u8; 32],
}

impl Ord for ContentKey {
    fn cmp(&self, other: &Self) -> Ordering {
        // Explicitly ordered values precede unordered ones, as do labelled.
        let order = match (self.order, other.order) {
            (Some(a), Some(b)) => a.0.total_cmp(&b.0),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        let label = match (&self.label, &other.label) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        order
            .then(label)
            .then_with(|| self.digest.cmp(&other.digest))
    }
}
impl PartialOrd for ContentKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for ContentKey {
    fn eq(&self, other: &Self) -> bool { self.cmp(other) == Ordering::Equal }
}
impl Eq for ContentKey {}

/// Position of a value among the values of the same predicate. Named and
/// literal values keep their query order and precede anonymous values.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Rank {
    Plain(usize),
    Anonymous(ContentKey),
}

/// Presentation key of any RDF value: named and literal values by their
/// N-Triples text, before anonymous values by their content.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum TermKey {
    Plain(String),
    Anonymous(ContentKey),
}
