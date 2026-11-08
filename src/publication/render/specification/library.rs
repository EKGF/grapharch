//! Library landing-page presentation derived only from the pinned plan and KG.
use {
    super::{
        super::{content::Entries, markup::html_escape, model::Plan},
        pages::{Catalog, subjects},
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, HashMap, HashSet},
};

#[derive(Default)]
struct Card {
    summary:    String,
    classes:    usize,
    properties: usize,
}

pub(super) struct Home {
    pub fragment:     String,
    pub introduction: String,
    pub status:       String,
    cards:            String,
}

impl Home {
    pub async fn read(plan: &Plan, catalog: &Catalog) -> Result<Option<Self>> {
        if !catalog.pages.iter().any(|page| page.kind == "ontology") {
            return Ok(None);
        }
        let Some(Term::NamedNode(family)) = plan.source.optional_by(&plan.profile, &NamedNode::new_unchecked("https://ekgf.org/ontology/artifact-publication#publishesFamily")).await? else { return Ok(None); };
        // bounded: retain only relevant explicit types, capped by the same
        // source assertion budget used by the ontology reference documentor.
        let mut types: HashMap<NamedNode, (bool, bool)> = HashMap::new();
        let pattern = plan.source.pattern("?subject a ?kind", 0);
        let mut rows = plan.dataset.select(format!("SELECT DISTINCT ?subject ?kind WHERE {{ {pattern} FILTER(isIRI(?subject)) VALUES ?kind {{ <http://www.w3.org/2002/07/owl#Class> <http://www.w3.org/2000/01/rdf-schema#Class> <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> <http://www.w3.org/2002/07/owl#ObjectProperty> <http://www.w3.org/2002/07/owl#DatatypeProperty> <http://www.w3.org/2002/07/owl#AnnotationProperty> }} }} LIMIT {}", crate::publication::limits::SOURCE_TYPE_ASSERTIONS+1)).await?;
        let mut assertions = 0;
        while let Some(row) = rows.next().await? {
            assertions += 1;
            ensure!(
                assertions <=
                    crate::publication::limits::SOURCE_TYPE_ASSERTIONS,
                "library type assertion budget exceeded"
            );
            if let (Term::NamedNode(subject), Term::NamedNode(kind)) =
                (&row["subject"], &row["kind"])
            {
                let flags = types.entry(subject.clone()).or_default();
                if kind.as_str().ends_with("#Class") {
                    flags.0 = true;
                } else {
                    flags.1 = true;
                }
            }
        }
        // bounded: one card per admitted specification page, one metric
        // membership per admitted fragment subject. Never count type rows
        // twice.
        let mut cards: BTreeMap<usize, Card> = BTreeMap::new();
        let mut seen = HashSet::new();
        let mut fragment = None;
        let mut introduction = String::new();
        let mut status = String::new();
        let mut entries = Entries::new(plan).await?;
        while let Some(entry) = entries.next(plan).await? {
            let page = catalog.placements[entry.placement.as_str()];
            let described = subjects(&entry, plan).await?;
            if page == 0 && matches!(entry.kind.as_str(), "abstract" | "status")
            {
                let rendered = super::pages::body(
                    &entry,
                    plan,
                    &mut super::outline::Outline::default(),
                )
                .await?;
                let (rendered, errors) =
                    super::pages::route_links(&rendered, catalog, page)?;
                ensure!(
                    errors.is_empty(),
                    "library introduction contains unresolved references"
                );
                let content = format!(
                    "<div id=\"{}\"><span id=\"{}\"></span>{rendered}</div>",
                    plan.anchor(&entry.placement, None),
                    plan.anchor(&entry.placement, Some(&entry.fragment))
                );
                if entry.kind == "abstract" {
                    introduction.push_str(&content);
                } else {
                    status.push_str(&content);
                }
            }
            if page == 0 && described.contains(&family) {
                fragment = Some(entry.fragment.as_str().to_owned());
            }
            if catalog.pages[page].kind != "ontology" {
                continue;
            }
            let card = cards.entry(page).or_default();
            if entry.first {
                ensure!(
                    described.len() == 1,
                    "ontology page requires one ontology identity"
                );
                card.summary = summary(plan, &described[0]).await?;
            }
            for subject in described {
                ensure!(
                    seen.len() <
                        crate::publication::limits::SPECIFICATION_ANCHORS,
                    "library metric membership budget exceeded"
                );
                if !seen.insert((page, subject.clone())) {
                    continue;
                }
                if let Some((class, property)) = types.get(&subject) {
                    card.classes += usize::from(*class);
                    card.properties += usize::from(*property);
                }
            }
        }
        let Some(fragment) = fragment else {
            anyhow::bail!("library has no retained overview fragment");
        };
        let mut html = String::from(
            "<p class=\"library-metric-note\">Counts show distinct documented \
             resources explicitly declared as classes or properties; multiple \
             declarations of the same resource count once.</p>",
        );
        let mut groups: BTreeMap<Option<usize>, Vec<usize>> = BTreeMap::new();
        for index in cards.keys() {
            groups
                .entry(catalog.pages[*index].parent)
                .or_default()
                .push(*index);
        }
        for (parent, members) in groups {
            if let Some(parent) = parent {
                let mut ancestors = Vec::new();
                let mut cursor = Some(parent);
                while let Some(index) = cursor {
                    ensure!(
                        ancestors.len() < catalog.pages.len(),
                        "cyclic library hierarchy"
                    );
                    ancestors.push(index);
                    cursor = catalog.pages[index].parent;
                }
                ancestors.reverse();
                html.push_str("<h2 class=\"library-module\">");
                for (position, index) in ancestors.iter().enumerate() {
                    if position > 0 {
                        html.push_str(" / ");
                    }
                    let page = &catalog.pages[*index];
                    html.push_str(&format!(
                        "<a href=\"{}\">{}</a>",
                        html_escape(&page.route),
                        html_escape(&page.title)
                    ));
                }
                html.push_str("</h2>");
            }
            html.push_str("<div class=\"ontology-cards\">");
            for index in members {
                let page = &catalog.pages[index];
                let card = &cards[&index];
                html.push_str(&format!(
                    "<article class=\"ontology-card\" data-class-count=\"{}\" \
                     data-property-count=\"{}\"><h2><a href=\"{}\">{}</a></h2>",
                    card.classes,
                    card.properties,
                    html_escape(&page.route),
                    html_escape(&page.title)
                ));
                if !card.summary.is_empty() {
                    html.push_str(&format!(
                        "<p>{}</p>",
                        html_escape(&card.summary)
                    ));
                }
                html.push_str(&format!(
                    "<dl class=\"ontology-metrics\"><div><dt>Classes</\
                     dt><dd>{}</dd></div><div><dt>Properties</dt><dd>{}</dd></\
                     div></dl></article>",
                    card.classes, card.properties
                ));
            }
            html.push_str("</div>");
        }
        html.push_str(
            "<nav class=\"library-resources\" aria-label=\"Library \
             information\">",
        );
        for page in catalog
            .pages
            .iter()
            .skip(1)
            .filter(|page| page.kind == "section")
        {
            html.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                html_escape(&page.route),
                html_escape(&page.title)
            ));
        }
        html.push_str("</nav>");
        ensure!(
            html.len() <= crate::publication::limits::REFERENCE_INDEX_BYTES,
            "library cards exceed byte budget"
        );
        Ok(Some(Self {
            fragment,
            cards: html,
            introduction,
            status,
        }))
    }

    pub fn render(&self, original: &str) -> String {
        let document = scraper::Html::parse_fragment(original);
        // Preserve existing deep-link identities while replacing the original
        // list presentation with cards, not a second copy of the library.
        let mut html = String::new();
        for node in document.select(&scraper::Selector::parse("[id]").unwrap())
        {
            html.push_str(&format!(
                "<span id=\"{}\"></span>",
                html_escape(node.value().attr("id").unwrap())
            ));
        }
        if let Some(intro) = document
            .select(&scraper::Selector::parse("p").unwrap())
            .next()
        {
            html.push_str(&format!(
                "<div class=\"library-introduction\">{}</div>",
                intro.html()
            ));
        }
        html.push_str(&self.cards);
        html
    }
}

async fn summary(plan: &Plan, subject: &NamedNode) -> Result<String> {
    for predicate in
        crate::publication::document::description::ONTOLOGY_DESCRIPTIONS
    {
        let mut rows = plan
            .source
            .objects_by(subject, &NamedNode::new_unchecked(predicate))
            .await?;
        while let Some(row) = rows.next().await? {
            if let Term::Literal(value) = &row["value"] {
                if value
                    .language()
                    .is_some_and(|language| !language.starts_with("en"))
                {
                    continue;
                }
                let text = value
                    .value()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                return Ok(text);
            }
        }
    }
    Ok(String::new())
}
