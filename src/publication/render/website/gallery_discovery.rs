//! Bounded, source-authored discovery metadata for the publication gallery.
use {
    crate::publication::render::{markup::html_escape as esc, model::Plan},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeSet,
};

#[derive(Default)]
pub(super) struct Discovery {
    pub inputs:       BTreeSet<String>,
    pub capabilities: BTreeSet<String>,
    pub uses:         BTreeSet<String>,
    pub outputs:      BTreeSet<String>,
    pub credits:      String,
}
impl Discovery {
    pub async fn read(plan: &Plan, id: &NamedNode) -> Result<Self> {
        let mut value = Self::default();
        for (field, labels) in [
            ("showcaseInput", &mut value.inputs),
            ("showcaseCapability", &mut value.capabilities),
            ("showcaseUseCase", &mut value.uses),
        ] {
            let mut rows = plan.source.objects(id, field).await?;
            while let Some(row) = rows.next().await? {
                let Term::Literal(label) = &row["value"] else {
                    anyhow::bail!("{field} must be a label");
                };
                let label = label.value();
                ensure!(
                    !label.trim().is_empty() &&
                        label.len() <= 64 &&
                        !label.chars().any(|c| c.is_control() || c == '|'),
                    "invalid showcase discovery label"
                );
                ensure!(
                    labels.len() < 16,
                    "showcase exceeds 16 labels per facet"
                );
                labels.insert(label.to_owned());
            }
        }
        let source = plan
            .source
            .optional_by(
                id,
                &NamedNode::new_unchecked("http://purl.org/dc/terms/source"),
            )
            .await?;
        let rights = plan
            .source
            .optional_by(
                id,
                &NamedNode::new_unchecked("http://purl.org/dc/terms/rights"),
            )
            .await?;
        if let Some(source) = source {
            let Term::NamedNode(source) = source else {
                anyhow::bail!("showcase source must be an HTTP(S) IRI")
            };
            ensure!(
                source.as_str().starts_with("https://") ||
                    source.as_str().starts_with("http://"),
                "showcase source must be HTTP(S)"
            );
            value.credits.push_str(&format!(
                "<a href=\"{}\">Original source</a>",
                esc(source.as_str())
            ));
        }
        if let Some(rights) = rights {
            let Term::Literal(rights) = rights else {
                anyhow::bail!("showcase credits must be text")
            };
            ensure!(
                rights.value().len() <= 1024,
                "showcase credits exceed 1024 bytes"
            );
            value
                .credits
                .push_str(&format!("<p>{}</p>", esc(rights.value())));
        }
        Ok(value)
    }

    pub fn attributes(&self) -> String {
        [
            ("input", &self.inputs),
            ("capability", &self.capabilities),
            ("use", &self.uses),
            ("output", &self.outputs),
        ]
        .into_iter()
        .map(|(key, values)| {
            format!(
                " data-{key}=\"{}\"",
                esc(&values
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join("|"))
            )
        })
        .collect()
    }

    pub fn chips(&self) -> String {
        let chips = self
            .inputs
            .iter()
            .chain(&self.capabilities)
            .chain(&self.uses)
            .map(|label| format!("<li>{}</li>", esc(label)))
            .collect::<String>();
        format!(
            "<ul class=\"showcase-tags\" aria-label=\"Source formats, \
             capabilities and uses\">{chips}</ul>"
        )
    }

    pub fn merge(&mut self, other: &Self) {
        self.inputs.extend(other.inputs.iter().cloned());
        self.capabilities.extend(other.capabilities.iter().cloned());
        self.uses.extend(other.uses.iter().cloned());
        self.outputs.extend(other.outputs.iter().cloned());
    }

    pub fn controls(&self, count: usize) -> String {
        let filters = [
            ("input", "Source format", &self.inputs),
            ("capability", "Capability", &self.capabilities),
            ("use", "Use case", &self.uses),
            ("output", "Output", &self.outputs),
        ]
        .into_iter()
        .map(|(key, title, values)| {
            let options = values
                .iter()
                .map(|value| {
                    format!(
                        "<option value=\"{}\">{}</option>",
                        esc(value),
                        esc(value)
                    )
                })
                .collect::<String>();
            format!(
                "<label>{title}<select data-gallery-filter=\"{key}\"><option \
                 value=\"\">All</option>{options}</select></label>"
            )
        })
        .collect::<String>();
        format!(
            r#"<form class="showcase-discovery" role="search" aria-label="Find a showcase" hidden><label class="showcase-query" for="showcase-search">Find a showcase<input id="showcase-search" type="search" placeholder="Search by topic, source or capability" maxlength="200" autocomplete="off"></label><div class="showcase-filters">{filters}</div><div class="showcase-results"><p role="status" aria-live="polite" aria-atomic="true" data-gallery-count>{count} showcases</p><button type="reset">Clear filters</button></div></form><p data-gallery-empty hidden>No showcases match these filters. Try another search or clear the filters.</p>"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Discovery;

    #[test]
    fn labels_are_escaped_in_filters_and_card_attributes() {
        let mut discovery = Discovery::default();
        discovery.inputs.insert("A \"quoted\" <format>".into());
        discovery.outputs.insert("HTML".into());
        assert!(
            discovery
                .attributes()
                .contains("&quot;quoted&quot; &lt;format&gt;")
        );
        let controls = discovery.controls(1);
        assert!(
            controls.contains("value=\"A &quot;quoted&quot; &lt;format&gt;\"")
        );
        assert!(!controls.contains("<format>"));
        assert!(controls.contains("data-gallery-filter=\"output\""));
    }
}
