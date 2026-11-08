//! Explicit SHACL query literals retain code semantics outside prose tables.
use {
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
};
#[derive(Default)]
pub(super) struct Queries {
    values: Vec<(String, String)>, /* bounded: 64 queries and 256 KiB per
                                    * documented resource. */
    bytes:  usize,
}
impl Queries {
    pub fn capture(
        &mut self,
        predicate: &NamedNode,
        value: &Term,
    ) -> Result<Option<String>> {
        let kind = match predicate.as_str() {
            "http://www.w3.org/ns/shacl#select" => "SELECT",
            "http://www.w3.org/ns/shacl#ask" => "ASK",
            _ => return Ok(None),
        };
        let Term::Literal(value) = value else {
            anyhow::bail!("SHACL {kind} query must be a literal")
        };
        self.bytes += value.value().len();
        ensure!(
            self.values.len() < 64 && self.bytes <= 256 * 1024,
            "SHACL query code exceeds per-resource budget"
        );
        self.values.push((kind.into(), value.value().into()));
        let number = self.values.len();
        Ok(Some(format!(
            "[SPARQL {kind} query {number}](#sparql-query-{number})"
        )))
    }

    pub fn append(&self, body: &mut String, depth: usize) {
        for (index, (kind, source)) in self.values.iter().enumerate() {
            let number = index + 1;
            let source = super::query_display::format(source);
            let width = source
                .split(|ch| ch != '`')
                .map(str::len)
                .max()
                .unwrap_or(0)
                .max(2) +
                1;
            let fence = "`".repeat(width);
            body.push_str(&format!(
                "\n\n{} SPARQL query {number}\n\n**SHACL {kind} \
                 query.**\n\n{fence}sparql\n{source}",
                "#".repeat(depth)
            ));
            if !source.ends_with('\n') {
                body.push('\n');
            }
            body.push_str(&format!("{fence}\n"));
        }
    }
}
