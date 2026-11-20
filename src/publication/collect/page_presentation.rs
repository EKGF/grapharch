//! Project-page presentation hints from Markdown frontmatter, retained as RDF.
//!
//! Each hint names a heading anchor or an authored label and selects a
//! presentation the website renderer offers. Nothing here inspects the page
//! body; the renderer validates that each named anchor exists.
use {
    super::source::FileInput,
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Quad},
    serde::Deserialize,
    std::collections::BTreeMap,
};

/// Sections of one kind a page may select.
const SECTIONS: usize = 32;
/// Card presentations or timeline statuses per page.
const BINDINGS: usize = 128;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Hints {
    pub subject:          Option<String>,
    pub hero:             Option<String>,
    pub fragment:         Option<String>,
    #[serde(default)]
    pub fragments:        BTreeMap<String, String>,
    #[serde(default)]
    pub showcase_gallery: bool,
    #[serde(default)]
    panel_sections:       Vec<String>,
    #[serde(default)]
    feature_sections:     Vec<String>,
    #[serde(default)]
    resource_sections:    Vec<String>,
    #[serde(default)]
    timeline_sections:    Vec<String>,
    #[serde(default)]
    callout_sections:     Vec<String>,
    #[serde(default)]
    term_sections:        Vec<String>,
    #[serde(default)]
    compact_sections:     Vec<String>,
    #[serde(default)]
    card_icons:           BTreeMap<String, CardIcon>,
    #[serde(default)]
    timeline_statuses:    BTreeMap<String, Marker>,
}

/// The icon, tone and optional badge of one heading-identified card.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardIcon {
    name:  String,
    tone:  String,
    badge: Option<String>,
}

/// The icon and tone marking a timeline entry of one status.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    name: String,
    tone: String,
}

impl Hints {
    /// The presentation assertions about the page `fragment`.
    pub fn quads(
        &self,
        input: &FileInput,
        fragment: &NamedNode,
    ) -> Result<Vec<Quad>> {
        // bounded: at most SECTIONS per kind and BINDINGS per binding kind.
        let mut quads = Vec::new();
        for (property, anchors) in [
            ("panelSection", &self.panel_sections),
            ("featureSection", &self.feature_sections),
            ("resourceSection", &self.resource_sections),
            ("timelineSection", &self.timeline_sections),
            ("calloutSection", &self.callout_sections),
            ("termSection", &self.term_sections),
            ("compactSection", &self.compact_sections),
        ] {
            ensure!(
                anchors.len() <= SECTIONS,
                "page exceeds {SECTIONS} values of ga:{property}"
            );
            for anchor in anchors {
                quads.push(input.quad(
                    fragment,
                    property,
                    Literal::new_simple_literal(anchor),
                ));
            }
        }
        ensure!(
            self.card_icons.len() <= BINDINGS,
            "page exceeds {BINDINGS} card icons"
        );
        for (anchor, icon) in &self.card_icons {
            let binding = Self::binding(
                input,
                fragment,
                &mut quads,
                ("card-presentation", "cardPresentation"),
                anchor,
            );
            let mut values = vec![
                ("headingAnchor", anchor.as_str()),
                ("iconName", icon.name.as_str()),
                ("iconTone", icon.tone.as_str()),
            ];
            if let Some(badge) = &icon.badge {
                values.push(("badgeLabel", badge.as_str()));
            }
            Self::literals(input, &binding, &mut quads, &values);
        }
        ensure!(
            self.timeline_statuses.len() <= BINDINGS,
            "page exceeds {BINDINGS} timeline statuses"
        );
        for (label, marker) in &self.timeline_statuses {
            let binding = Self::binding(
                input,
                fragment,
                &mut quads,
                ("timeline-status", "timelineStatus"),
                label,
            );
            Self::literals(input, &binding, &mut quads, &[
                ("statusLabel", label.as_str()),
                ("iconName", marker.name.as_str()),
                ("iconTone", marker.tone.as_str()),
            ]);
        }
        Ok(quads)
    }

    /// A stable binding resource of the page, linked by `property`.
    fn binding(
        input: &FileInput,
        fragment: &NamedNode,
        quads: &mut Vec<Quad>,
        (kind, property): (&str, &str),
        key: &str,
    ) -> NamedNode {
        let binding = crate::publication::dataset::identity(kind, &[
            fragment.as_str(),
            key,
        ]);
        quads.push(input.quad(fragment, property, binding.clone()));
        binding
    }

    fn literals(
        input: &FileInput,
        subject: &NamedNode,
        quads: &mut Vec<Quad>,
        values: &[(&str, &str)],
    ) {
        for (predicate, value) in values {
            quads.push(input.quad(
                subject,
                predicate,
                Literal::new_simple_literal(*value),
            ));
        }
    }
}
