//! Explicit SHACL query literals retain code semantics outside prose tables.
use {
    super::query_caption::QueryCaption,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, NamedNodeRef, Term},
};

const SH_SELECT: NamedNodeRef<'_> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#select");
const SH_ASK: NamedNodeRef<'_> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#ask");

/// The SHACL query form a retained literal carries.
#[derive(Clone, Copy)]
enum QueryKind {
    Select,
    Ask,
}

impl QueryKind {
    fn of(predicate: &NamedNode) -> Option<Self> {
        let predicate = predicate.as_ref();
        if predicate == SH_SELECT {
            Some(Self::Select)
        } else if predicate == SH_ASK {
            Some(Self::Ask)
        } else {
            None
        }
    }

    fn keyword(self) -> &'static str {
        match self {
            Self::Select => "SELECT",
            Self::Ask => "ASK",
        }
    }
}

/// The resource whose annotations caption a query.
pub(super) enum QueryOwner<'a> {
    /// The documented term itself, whose facts are still streaming.
    Subject,
    /// An anonymous resource whose facts are already retained.
    Anonymous(&'a QueryCaption),
}

struct Query {
    kind:    QueryKind,
    source:  String,
    /// `None` defers to the documented term's own caption.
    caption: Option<Option<String>>,
}

#[derive(Default)]
pub(super) struct Queries {
    values: Vec<Query>, /* bounded: 64 queries and 256 KiB per documented
                         * resource. */
    bytes:  usize,
}
impl Queries {
    pub fn capture(
        &mut self,
        predicate: &NamedNode,
        value: &Term,
        owner: QueryOwner<'_>,
    ) -> Result<Option<String>> {
        let Some(kind) = QueryKind::of(predicate) else {
            return Ok(None)
        };
        let keyword = kind.keyword();
        let Term::Literal(value) = value else {
            anyhow::bail!("SHACL {keyword} query must be a literal")
        };
        self.bytes += value.value().len();
        ensure!(
            self.values.len() < 64 && self.bytes <= 256 * 1024,
            "SHACL query code exceeds per-resource budget"
        );
        self.values.push(Query {
            kind,
            source: value.value().into(),
            caption: match owner {
                QueryOwner::Subject => None,
                QueryOwner::Anonymous(caption) => Some(caption.markdown()),
            },
        });
        let number = self.values.len();
        Ok(Some(format!(
            "[SPARQL {keyword} query {number}](#sparql-query-{number})"
        )))
    }

    /// Append each query section; `subject` captions the term's own queries.
    pub fn append(
        &self,
        body: &mut String,
        depth: usize,
        subject: &QueryCaption,
    ) {
        let subject = subject.markdown();
        for (index, query) in self.values.iter().enumerate() {
            let number = index + 1;
            let source = super::query_display::format(&query.source);
            let width = source
                .split(|ch| ch != '`')
                .map(str::len)
                .max()
                .unwrap_or(0)
                .max(2) +
                1;
            let fence = "`".repeat(width);
            let caption = query
                .caption
                .clone()
                .unwrap_or_else(|| subject.clone())
                .unwrap_or_else(|| {
                    format!("**SHACL {} query.**", query.kind.keyword())
                });
            body.push_str(&format!(
                "\n\n{} SPARQL query \
                 {number}\n\n{caption}\n\n{fence}sparql\n{source}",
                "#".repeat(depth)
            ));
            if !source.ends_with('\n') {
                body.push('\n');
            }
            body.push_str(&format!("{fence}\n"));
        }
    }
}
