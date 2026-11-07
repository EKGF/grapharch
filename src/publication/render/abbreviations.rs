//! Bounded lexical presentation from source-owned, pinned SKOS dictionaries.
use {
    super::{markup::html_escape, model::Plan},
    anyhow::{Result, ensure},
    oxrdf::{Literal, Term},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    regex::Regex,
    std::collections::BTreeMap,
};
const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
#[derive(Default)]
pub(super) struct Abbreviations {
    definitions: BTreeMap<String, String>, /* bounded: 1024 terms, 128 KiB
                                            * total. */
    pattern:     Option<Regex>,
}
impl Abbreviations {
    pub async fn read(plan: &Plan) -> Result<Self> {
        let mut definitions = BTreeMap::new();
        let mut bytes = 0usize;
        let mut schemes = plan
            .source
            .objects(&plan.project, "abbreviationScheme")
            .await?;
        let mut count = 0;
        let mut term_rows = 0usize;
        while let Some(row) = schemes.next().await? {
            count += 1;
            ensure!(
                count <= 8,
                "project exceeds eight abbreviation schemes"
            );
            let Term::NamedNode(scheme) = &row["value"] else {
                anyhow::bail!("abbreviation scheme must be an IRI")
            };
            let typed = plan
                .source
                .pattern(&format!("{scheme} a <{SKOS}ConceptScheme>"), 0);
            let mut exists = plan
                .dataset
                .select(format!("SELECT * WHERE {{ {typed} }} LIMIT 1"))
                .await?;
            ensure!(
                exists.next().await?.is_some(),
                "abbreviation scheme lacks its SKOS type: {scheme}"
            );
            let members = plan
                .source
                .pattern(&format!("?concept <{SKOS}inScheme> {scheme}"), 0);
            let notation = plan
                .source
                .pattern(&format!("?concept <{SKOS}notation> ?short"), 1);
            let definition = plan.source.pattern(
                &format!("?concept <{SKOS}definition> ?definition"),
                2,
            );
            let typed = plan
                .source
                .pattern(&format!("?concept a <{SKOS}Concept>"), 3);
            let mut rows = plan
                .dataset
                .select(format!(
                    "SELECT DISTINCT ?concept ?short ?definition ?typed WHERE \
                     {{ {members} OPTIONAL {{ {notation} }} OPTIONAL {{ \
                     {definition} }} BIND(EXISTS {{ {typed} }} AS ?typed) }} \
                     ORDER BY ?concept ?short ?definition"
                ))
                .await?;
            while let Some(row) = rows.next().await? {
                term_rows += 1;
                ensure!(
                    term_rows <= 8192,
                    "abbreviation scheme membership exceeds budget"
                );
                ensure!(
                    row.get("typed") ==
                        Some(&Term::Literal(Literal::from(true))),
                    "abbreviation must be a SKOS Concept"
                );
                let (Some(Term::Literal(short)), Some(Term::Literal(long))) =
                    (row.get("short"), row.get("definition"))
                else {
                    anyhow::bail!(
                        "abbreviation needs literal notation and definition"
                    )
                };
                let (short, long) = (short.value(), long.value());
                ensure!(
                    !short.trim().is_empty() &&
                        short.len() <= 128 &&
                        !short.chars().any(char::is_control) &&
                        !long.trim().is_empty() &&
                        long.len() <= 4096,
                    "abbreviation exceeds term or definition bounds"
                );
                if let Some(previous) = definitions.get(short) {
                    ensure!(
                        previous == long,
                        "conflicting abbreviation definition: {short}"
                    );
                    continue;
                }
                bytes += short.len() + long.len();
                ensure!(
                    definitions.len() < 1024 && bytes <= 128 * 1024,
                    "abbreviation dictionary exceeds 1024 terms or 128 KiB"
                );
                definitions.insert(short.to_owned(), long.to_owned());
            }
        }
        let pattern = if definitions.is_empty() {
            None
        } else {
            let mut words: Vec<_> = definitions.keys().collect(); // bounded: dictionary limit above.
            words.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
            Some(Regex::new(
                &words
                    .into_iter()
                    .map(|word| regex::escape(word))
                    .collect::<Vec<_>>()
                    .join("|"),
            )?)
        };
        Ok(Self { definitions, pattern })
    }

    /// Remove only configured dictionary definitions included in Markdown
    /// prose.
    pub fn clean(&self, body: &str) -> Result<String> {
        if self.definitions.is_empty() {
            return Ok(body.to_owned());
        }
        let protected: Vec<_> =
            Parser::new_ext(body, crate::publication::markdown::options())
                .into_offset_iter()
                .filter_map(|(event, range)| {
                    matches!(event, Event::Start(Tag::CodeBlock(_)))
                        .then_some(range)
                })
                .collect(); // bounded: one capped Markdown body.
        let mut offset = 0;
        let mut protected_index = 0usize;
        let mut out = String::new();
        for line in body.split_inclusive('\n') {
            while protected
                .get(protected_index)
                .is_some_and(|range| range.end <= offset)
            {
                protected_index += 1;
            }
            let protected =
                protected.get(protected_index).is_some_and(|range| {
                    range.start < offset + line.len() && offset < range.end
                });
            offset += line.len();
            let definition = (!protected)
                .then(|| {
                    line.trim()
                        .strip_prefix("*[")
                        .and_then(|value| value.split_once("]:"))
                })
                .flatten();
            if let Some((short, long)) = definition {
                ensure!(
                    self.definitions
                        .get(short)
                        .is_some_and(|known| known == long.trim()),
                    "included abbreviation definition is missing or conflicts \
                     with configured dictionary: {short}"
                );
                out.push('\n');
            } else {
                out.push_str(line);
            }
        }
        Ok(out)
    }

    pub fn html<'a>(
        &self,
        events: impl Iterator<Item = Event<'a>>,
    ) -> Result<Vec<Event<'a>>> {
        let Some(pattern) = &self.pattern else {
            return Ok(events.collect());
        };
        let mut output = Vec::new(); // bounded: one capped page plus at most 16384 abbreviation occurrences.
        let mut blocked = 0usize;
        let mut occurrences = 0usize;
        let mut bytes = 0usize;
        for event in events {
            match &event {
                Event::Start(Tag::Image { .. } | Tag::CodeBlock(_)) => {
                    blocked += 1
                },
                Event::End(TagEnd::Image | TagEnd::CodeBlock) => {
                    blocked = blocked.saturating_sub(1)
                },
                _ => {},
            }
            let Event::Text(text) = &event else {
                output.push(event);
                continue;
            };
            if blocked != 0 {
                output.push(event);
                continue;
            }
            let mut html = String::new();
            let mut end = 0;
            for found in pattern.find_iter(text) {
                let word = |ch: char| ch.is_alphanumeric() || ch == '_';
                if text[..found.start()].chars().next_back().is_some_and(word) ||
                    text[found.end()..].chars().next().is_some_and(word)
                {
                    continue;
                }
                occurrences += 1;
                ensure!(
                    occurrences <= 16384,
                    "Markdown exceeds abbreviation occurrence budget"
                );
                html.push_str(&html_escape(&text[end..found.start()]));
                html.push_str(&format!(
                    "<abbr title=\"{}\">{}</abbr>",
                    html_escape(&self.definitions[found.as_str()]),
                    html_escape(found.as_str())
                ));
                end = found.end();
            }
            if end == 0 {
                output.push(event);
                continue;
            }
            html.push_str(&html_escape(&text[end..]));
            bytes += html.len();
            ensure!(
                bytes <= 8 * 1024 * 1024,
                "abbreviation presentation exceeds 8 MiB"
            );
            output.push(Event::InlineHtml(html.into()));
        }
        Ok(output)
    }
}
