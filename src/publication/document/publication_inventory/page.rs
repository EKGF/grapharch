//! The generated Markdown of a publication version page.
//!
//! The page is an ordinary project page: an optional eyebrow, the title, the
//! page description as its lead, then one linked card per advertised version
//! in a resource list. A version's declared icon, tone and status become the
//! card's presentation, so the engine holds no status vocabulary of its own.
use {
    super::{model::Job, version::Version},
    crate::publication::collect::page::slug,
    anyhow::Result,
    serde::Serialize,
    std::collections::{BTreeMap, HashMap},
};

#[derive(Serialize)]
struct Frontmatter {
    nav_order: u32,
    grapharch: Hints,
}

#[derive(Serialize)]
struct Hints {
    resource_sections: [String; 1],
    // bounded: one entry per advertised version of a capped inventory.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    card_icons:        BTreeMap<String, CardIcon>,
}

#[derive(Serialize)]
struct CardIcon {
    name:  String,
    tone:  String,
    #[serde(skip_serializing_if = "Option::is_none")]
    badge: Option<String>,
}

/// Heading anchors, numbered on repetition exactly as the renderer does.
#[derive(Default)]
struct Anchors(
    // bounded: one entry per heading of a capped inventory page.
    HashMap<String, usize>,
);

impl Anchors {
    fn next(&mut self, title: &str) -> String {
        let base = slug(title);
        let occurrence = self.0.entry(base.clone()).or_insert(0);
        let anchor = if *occurrence == 0 {
            base
        } else {
            format!("{base}-{occurrence}")
        };
        *occurrence += 1;
        anchor
    }
}

/// The inputs of one generated version page.
pub(super) struct VersionPage<'a> {
    pub job:         &'a Job,
    pub description: Option<&'a str>,
    pub versions:    &'a [Version],
}

impl VersionPage<'_> {
    pub fn markdown(&self) -> Result<String> {
        let mut anchors = Anchors::default();
        let mut hints = Hints {
            resource_sections: [anchors.next(&self.job.title)],
            card_icons:        BTreeMap::new(),
        };
        let mut body = String::new();
        if let Some(eyebrow) = &self.job.eyebrow {
            body.push_str(&format!("{}\n\n", text(eyebrow)));
        }
        body.push_str(&format!("# {}\n\n", text(&self.job.title)));
        if let Some(description) = self.description {
            body.push_str(&format!("{}\n\n", text(description)));
        }
        for version in self.versions.iter().filter(|version| version.advertised)
        {
            let anchor = anchors.next(&version.label);
            if let (Some(name), Some(tone)) = (&version.icon, &version.tone) {
                hints.card_icons.insert(anchor, CardIcon {
                    name:  name.clone(),
                    tone:  tone.clone(),
                    badge: version.status.clone(),
                });
            }
            body.push_str(&Self::card(version));
        }
        let frontmatter = serde_yaml::to_string(&Frontmatter {
            nav_order: self.job.order,
            grapharch: hints,
        })?;
        Ok(format!("---\n{frontmatter}---\n{body}"))
    }

    /// One version: its linked title, description and location line.
    fn card(version: &Version) -> String {
        let mut card = format!(
            "### [{}](<{}>)\n\n",
            text(&version.label),
            version.url
        );
        if let Some(description) = &version.description {
            card.push_str(&format!("{}\n\n", text(description)));
        }
        // Double-backtick spans hold any branch name verbatim.
        let mut location = format!("`` {} ``", version.location());
        if version.branch != version.slug {
            location.push_str(&format!(" · branch `` {} ``", version.branch));
        }
        // Without an icon the status cannot be a badge; it joins this line.
        if let (None, Some(status)) = (&version.icon, &version.status) {
            location.push_str(&format!(" · {}", text(status)));
        }
        card.push_str(&format!("*{location}*\n\n"));
        card
    }
}

/// Escape Markdown syntax in plain text.
fn text(value: &str) -> String {
    let mut output = String::new();
    for c in value.chars() {
        if "\\`*_{}[]<>#|!".contains(c) {
            output.push('\\');
        }
        output.push(c);
    }
    output
}
