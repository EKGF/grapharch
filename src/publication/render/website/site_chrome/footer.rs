//! An authored website footer: ordered groups of text or links, then ordered
//! notes. When a profile declares one it replaces the theme's footer.
use {
    super::{
        footer_link::{FooterLink, order, title},
        footer_markdown::FooterText,
    },
    crate::publication::render::{markup::html_escape as esc, model::Plan},
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

const DESCRIPTION: &str = "http://purl.org/dc/terms/description";
const MAX_GROUPS: usize = 6;
const MAX_LINKS: usize = 16;
const MAX_NOTES: usize = 4;

pub(in crate::publication::render::website) struct SiteFooter {
    // bounded: at most MAX_GROUPS groups and MAX_NOTES notes.
    groups: Vec<Group>,
    notes:  Vec<(u32, FooterText)>,
}

struct Group {
    order: u32,
    title: String,
    body:  GroupBody,
}

enum GroupBody {
    Text(FooterText),
    // bounded: at most MAX_LINKS links.
    Links(Vec<FooterLink>),
}

impl SiteFooter {
    pub async fn read(plan: &Plan) -> Result<Option<Self>> {
        let footer =
            match plan.source.optional(&plan.profile, "siteFooter").await? {
                None => return Ok(None),
                Some(Term::NamedNode(footer)) => footer,
                Some(_) => bail!("ga:siteFooter must be a footer IRI"),
            };
        ensure!(
            plan.source.has_type(&footer, "SiteFooter").await?,
            "{footer} is not a declared ga:SiteFooter"
        );
        let mut groups = Vec::new();
        let mut rows = plan.source.objects(&footer, "footerGroup").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                groups.len() < MAX_GROUPS,
                "footer exceeds {MAX_GROUPS} groups"
            );
            groups.push(Group::read(plan, &row["value"]).await?);
        }
        ensure!(
            !groups.is_empty(),
            "{footer} requires a ga:footerGroup"
        );
        let mut notes = Vec::new();
        let mut rows = plan.source.objects(&footer, "footerNote").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                notes.len() < MAX_NOTES,
                "footer exceeds {MAX_NOTES} notes"
            );
            let note = identified(&row["value"], "footer note")?;
            notes.push((
                order(plan, &note, "footer note").await?,
                FooterText::try_new(
                    description(plan, &note).await?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "footer note {note} requires dct:description"
                        )
                    })?,
                )?,
            ));
        }
        groups.sort_by_key(|group| group.order);
        notes.sort_by_key(|(order, _)| *order);
        distinct(
            groups.iter().map(|group| group.order),
            "footer groups",
        )?;
        distinct(
            notes.iter().map(|(order, _)| *order),
            "footer notes",
        )?;
        Ok(Some(Self { groups, notes }))
    }

    /// The footer, closed by the engine's generator credit and provenance.
    pub fn html(&self, prefix: &str) -> Result<String> {
        let mut html = String::from(
            "<footer class=\"site-footer\"><div \
             class=\"site-footer-inner\"><div class=\"site-footer-grid\">",
        );
        for group in &self.groups {
            html.push_str(&group.html(prefix)?);
        }
        html.push_str("</div><div class=\"site-footer-notes\">");
        for (_, note) in &self.notes {
            html.push_str(&format!("<p>{}</p>", note.html(prefix)?));
        }
        html.push_str(&format!(
            "<p class=\"site-footer-credit\">{} · <a \
             href=\"{prefix}provenance.html\">Provenance</a></p></div></div></\
             footer>",
            super::super::super::credit::html()
        ));
        Ok(html)
    }
}

impl Group {
    async fn read(plan: &Plan, value: &Term) -> Result<Self> {
        let group = identified(value, "footer group")?;
        let mut links = Vec::new();
        let mut rows = plan.source.objects(&group, "footerLink").await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                links.len() < MAX_LINKS,
                "footer group exceeds {MAX_LINKS} links"
            );
            links.push(FooterLink::read(plan, &row["value"]).await?);
        }
        let body = match (description(plan, &group).await?, links.is_empty()) {
            (Some(text), true) => GroupBody::Text(FooterText::try_new(text)?),
            (None, false) => {
                links.sort_by_key(FooterLink::order);
                distinct(
                    links.iter().map(FooterLink::order),
                    "footer links",
                )?;
                GroupBody::Links(links)
            },
            _ => {
                bail!(
                    "footer group {group} requires exactly one of \
                     dct:description or ga:footerLink"
                )
            },
        };
        Ok(Self {
            order: order(plan, &group, "footer group").await?,
            title: title(plan, &group).await?,
            body,
        })
    }

    fn html(&self, prefix: &str) -> Result<String> {
        let title = esc(&self.title);
        Ok(match &self.body {
            GroupBody::Text(text) => {
                format!(
                    "<section class=\"site-footer-group\"><h2>{title}</\
                     h2><p>{}</p></section>",
                    text.html(prefix)?
                )
            },
            GroupBody::Links(links) => {
                format!(
                    "<nav class=\"site-footer-group\" \
                     aria-label=\"{title}\"><h2>{title}</h2><ul>{}</ul></nav>",
                    links
                        .iter()
                        .map(|link| link.html(prefix))
                        .collect::<String>()
                )
            },
        })
    }
}

fn identified(value: &Term, kind: &str) -> Result<NamedNode> {
    match value {
        Term::NamedNode(node) => Ok(node.clone()),
        _ => bail!("{kind} must be an identified resource"),
    }
}

async fn description(
    plan: &Plan,
    subject: &NamedNode,
) -> Result<Option<String>> {
    match plan
        .source
        .optional_by(subject, &NamedNode::new_unchecked(DESCRIPTION))
        .await?
    {
        None => Ok(None),
        Some(Term::Literal(text)) => Ok(Some(text.value().to_owned())),
        Some(_) => bail!("{subject} dct:description must be a literal"),
    }
}

fn distinct(orders: impl Iterator<Item = u32>, kind: &str) -> Result<()> {
    let mut previous = None;
    for order in orders {
        ensure!(
            previous != Some(order),
            "{kind} require distinct orders"
        );
        previous = Some(order);
    }
    Ok(())
}
